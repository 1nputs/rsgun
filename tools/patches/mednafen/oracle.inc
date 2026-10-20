/* stv-recomp oracle hooks for Mednafen's Saturn/ST-V core (included in src/ss/ss.cpp before debug.inc).
 *
 * A development tool only: Mednafen is GPL-2+, and nothing here is shipped or linked into the port.
 * Driven by environment variables so the stock command line still works:
 *
 *   STV_ORACLE_DIR    directory to write into (the hooks are off when it is unset)
 *   STV_ORACLE_DUMP   frames to dump at, comma separated ("600,1200")
 *   STV_ORACLE_EXIT   exit after this frame (after its dump and trace, if any)
 *   STV_ORACLE_TRACE  F:N[,F:N...]  trace both SH-2s through frames F to F+N-1 (windows in order; a
 *                     window from frame 1 starts at power on)
 *   STV_ORACLE_SOUND  N  log the SCSP from power on through frame N to DIR/sound/ (below)
 *   STV_ORACLE_INPUT  FILE  the I/O chip's input ports from a script: lines "F P" from frame F on (the
 *                     F-th Emulate() call, counting from 0), P sixteen hex digits, ports A-H
 *                     (active low); port D stays as the oracle has it
 *   STV_ORACLE_HASHES FILE  write a line per frame: the frame number, then 64-bit FNV-1a hashes (hex) of
 *                     the big-endian bytes of work RAM-L, work RAM-H, VDP1 VRAM, VDP2 VRAM, VDP2 CRAM,
 *                     VDP2's registers (as written, 0x200 bytes) and sound RAM, then both SH-2s' PCs
 *
 * Frames count Emulate() calls from 1. A dump at frame F (taken after the frame) is DIR/fFFFFFF/:
 *   wram-l.bin wram-h.bin      work RAM, 1 MB each, big-endian bytes as the SH-2 sees them
 *   vdp1-vram.bin              VDP1 VRAM (512 KB)
 *   vdp2-vram.bin vdp2-cram.bin
 *   sound-ram.bin              SCSP RAM (512 KB)
 *   eeprom.bin                 the ST-V's EEPROM (64 words)
 *   regs.txt                   both SH-2s' registers, and the frame's geometry
 *   regs.bin                   the same registers: per CPU (master, slave) 23 big-endian words:
 *                              R0-R15, PC, PR, SR, GBR, VBR, MACH, MACL; then the sound CPU (68000):
 *                              19 words, D0-D7, A0-A7, PC, SR, the inactive stack pointer
 *   screen.ppm                 the frame as displayed (DisplayRect, the widest line's width)
 *   display/                   the state the frame was displayed from, taken at its VBlank-OUT (before
 *                              display line 0): vdp2-regs.bin (VDP2's 0x200 bytes of registers as
 *                              written), vdp2-vram.bin, vdp2-cram.bin, vdp1-fb.bin (the framebuffer VDP1
 *                              has just drawn, which the frame change at this VBlank-OUT puts on display:
 *                              256 KB), vdp1-regs.bin (TVMR, FBCR: two bytes)
 *   draw/                      the first VDP1 draw that starts in the frame: vdp1-vram.bin and fb-before.bin
 *                              at its start (the framebuffer it draws into), fb-after.bin at its end,
 *                              vdp1-regs.bin (TVMR, FBCR, PTMR, EWDR, EWLR, EWRR: six words)
 *
 * A trace from frame F is DIR/tFFFFFF/: start/ (a dump taken before frame F, the state the trace
 * starts from) and m.trace, s.trace, k.trace (the master's, the slave's and the sound CPU's records,
 * in the order the oracle made them). Version 3 records are 12 bytes, big-endian:
 *   u8 tag; u8 size; u16 0; u32 a; u32 v
 *   'P' a step: a = the PC the debugger reports before the CPU's step (0 when it has none), size = the
 *       pipeline's operation index (Pipe_ID >> 24), v = the instruction word. Indices 0x7E (standby),
 *       0xFE (stalled for a DMA burst or an external halt) and 0x7F/0xFF (an exception pending: an 'X'
 *       follows if one is taken, else the instruction runs in the same step) are Mednafen's pseudo-ops
 *   'R' a data read (instruction fetches are not recorded): size 1, 2 or 4, a = address, v = value
 *   'W' a data write: the same
 *   'X' an exception taken: size = vector number, a = the PC saved on the stack, v = exception number
 *       (Mednafen's: 0 power on and 1 reset, which save nothing (a = 0), 9 an interrupt)
 *   'I' an interrupt's level, after its 'X' and the stack and vector accesses: size = the level (SR.I)
 *   'F' the end of a frame: a = the frame number
 * In k.trace (the 68000): 'P' a = the instruction's address, v = its first word (size 0); 'R'/'W'
 * its bus accesses (8 or 16 bits: a long is two words); 'X' an exception: size = vector number
 * (before an interrupt's acknowledge), a = the PC saved, v = Mednafen's exception number (0xFF a
 * reset, with a = the new PC); 'I' an interrupt's level (size) and the vector taken (a).
 * The file begins with one header record: 'H', size 3 (the version), a = 0x53545654 ("STVT"), v = 0.
 *
 * The sound log, DIR/sound/: samples.bin, each sample the SCSP makes (44.1 kHz), as two big-endian
 * 16-bit words, left and right, before Mednafen scales its output; writes.bin, 16-byte big-endian
 * records: u32 samples made before it; u8 tag; u8 size; u16 0; u32 a; u32 v
 *   'W' a write into the SCSP from either CPU or a DMA: a = the SCSP's address (RAM from 0, the
 *       registers from 0x100000), v = the value (size 1 or 2)
 *   'F' the end of a frame: a = the frame number
 */

#include <sys/stat.h>
#include <array>

namespace VDP1	// (declared only inside inlines in vdp1.h, or in vdp1_common.h)
{
 MDFN_HIDE extern uint16 VRAM[0x40000];
 MDFN_HIDE extern uint16 FB[2][0x20000];
 MDFN_HIDE extern uint8 TVMR;
 MDFN_HIDE extern uint8 FBCR;
}

static struct
{
 bool on;
 std::string dir;
 std::vector<uint64> dumps;
 uint64 exit_frame;
 uint64 frame;
 std::vector<std::pair<uint64, uint64>> traces;	// windows of frames [first, first + count)
 size_t trace_next;	// the next window to start
 uint64 trace_end;	// the frame the current window ends after
 bool tracing;
 bool draw_started;	// a draw caught by the draw hook has started and not ended
 FILE* hashes_fp;
 FILE* trace_fp[3];
 std::vector<uint8> trace_buf[3];
 std::vector<std::pair<uint64, std::array<uint8, 8>>> input;	// from frame, the ports
 uint64 sound_end;	// the frame the sound log ends after (0: none)
 FILE* sound_fp[2];	// writes, samples
} Oracle;

static INLINE bool Oracle_Tracing(void)
{
 return Oracle.tracing;
}

static void Oracle_Record(unsigned cpu, uint8 tag, uint8 size, uint32 a, uint32 v)
{
 std::vector<uint8>& b = Oracle.trace_buf[cpu];
 const uint8 rec[12] = { tag, size, 0, 0, (uint8)(a >> 24), (uint8)(a >> 16), (uint8)(a >> 8), (uint8)a,
			 (uint8)(v >> 24), (uint8)(v >> 16), (uint8)(v >> 8), (uint8)v };

 b.insert(b.end(), rec, rec + sizeof(rec));
 if(b.size() >= (1U << 22))
 {
  fwrite(b.data(), 1, b.size(), Oracle.trace_fp[cpu]);
  b.clear();
 }
}

static INLINE void Oracle_Step(unsigned cpu, uint32 pc, uint32 pipe_id)
{
 if(MDFN_UNLIKELY(Oracle.tracing))
  Oracle_Record(cpu, 'P', pipe_id >> 24, pc, (uint16)pipe_id);
}

static INLINE void Oracle_Mem(unsigned cpu, uint8 kind, unsigned size, uint32 a, uint32 v)
{
 if(MDFN_UNLIKELY(Oracle.tracing))
  Oracle_Record(cpu, kind, size, a, v);
}

static INLINE void Oracle_Exception(unsigned cpu, unsigned exnum, unsigned vecnum, uint32 saved_pc)
{
 if(MDFN_UNLIKELY(Oracle.tracing))
  Oracle_Record(cpu, 'X', vecnum, saved_pc, exnum);
}

static INLINE void Oracle_IntLevel(unsigned cpu, unsigned level)
{
 if(MDFN_UNLIKELY(Oracle.tracing))
  Oracle_Record(cpu, 'I', level, 0, 0);
}

static void Oracle_SoundHook(uint8 tag, uint8 size, uint32 a, uint32 v)
{
 Oracle_Record(2, tag, size, a, v);
}

static void Oracle_SoundRecord(uint64 samples, uint8 tag, uint8 size, uint32 a, uint32 v)
{
 const uint8 rec[16] = { (uint8)(samples >> 24), (uint8)(samples >> 16), (uint8)(samples >> 8), (uint8)samples,
			 tag, size, 0, 0, (uint8)(a >> 24), (uint8)(a >> 16), (uint8)(a >> 8), (uint8)a,
			 (uint8)(v >> 24), (uint8)(v >> 16), (uint8)(v >> 8), (uint8)v };

 fwrite(rec, 1, sizeof(rec), Oracle.sound_fp[0]);
}

static void Oracle_SoundWrite(uint64 samples, unsigned size, uint32 a, uint32 v)
{
 Oracle_SoundRecord(samples, 'W', size, a, size == 1 ? (uint8)v : (uint16)v);
}

static void Oracle_SoundSample(const int16* lr)
{
 const uint8 s[4] = { (uint8)((uint16)lr[0] >> 8), (uint8)lr[0], (uint8)((uint16)lr[1] >> 8), (uint8)lr[1] };

 fwrite(s, 1, sizeof(s), Oracle.sound_fp[1]);
}

static void Oracle_Input(uint8* data_in)
{
 const std::array<uint8, 8>* ports = nullptr;

 for(const auto& l : Oracle.input)
 {
  if(l.first > Oracle.frame)
   break;
  ports = &l.second;
 }
 for(unsigned i = 0; i < 8; i++)
 {
  if(i != 3)
   data_in[i] = ports ? (*ports)[i] : 0xFF;
 }
}

static void Oracle_Init(void)
{
 const char* dir = getenv("STV_ORACLE_DIR");

 Oracle.on = (dir && *dir);
 Oracle.tracing = false;
 Oracle.hashes_fp = nullptr;
 if(!Oracle.on)
  return;
 if(const char* h = getenv("STV_ORACLE_HASHES"))
 {
  if(*h)
   Oracle.hashes_fp = fopen(h, "w");
 }
 Oracle.dir = dir;
 Oracle.frame = 0;
 Oracle.exit_frame = 0;
 Oracle.traces.clear();
 Oracle.trace_next = 0;
 Oracle.dumps.clear();
 if(const char* d = getenv("STV_ORACLE_DUMP"))
 {
  for(const char* p = d; *p;)
  {
   char* end;
   const unsigned long long f = strtoull(p, &end, 10);

   if(end == p)
    break;
   Oracle.dumps.push_back(f);
   p = (*end == ',') ? end + 1 : end;
  }
 }
 if(const char* e = getenv("STV_ORACLE_EXIT"))
  Oracle.exit_frame = strtoull(e, nullptr, 10);
 Oracle.input.clear();
 if(const char* in = getenv("STV_ORACLE_INPUT"))
 {
  if(FILE* fp = (*in ? fopen(in, "r") : nullptr))
  {
   unsigned long long f, p;

   while(fscanf(fp, "%llu %llx", &f, &p) == 2)
   {
    std::array<uint8, 8> ports;

    for(unsigned i = 0; i < 8; i++)
     ports[i] = (uint8)(p >> (56 - 8 * i));
    Oracle.input.push_back({ f, ports });
   }
   fclose(fp);
   STVIO_OracleInput = Oracle_Input;
  }
 }
 Oracle.sound_end = 0;
 if(const char* s = getenv("STV_ORACLE_SOUND"))
  Oracle.sound_end = strtoull(s, nullptr, 10);
 if(const char* t = getenv("STV_ORACLE_TRACE"))
 {
  for(const char* p = t; *p;)
  {
   char* end;
   const uint64 first = strtoull(p, &end, 10);
   uint64 count = 1;

   if(end == p)
    break;
   if(*end == ':')
    count = strtoull(end + 1, &end, 10);
   Oracle.traces.push_back({ first, count });
   p = (*end == ',') ? end + 1 : end;
  }
 }
 mkdir(Oracle.dir.c_str(), 0755);
 if(Oracle.sound_end)
 {
  const std::string d = Oracle.dir + "/sound";

  mkdir(d.c_str(), 0755);
  Oracle.sound_fp[0] = fopen((d + "/writes.bin").c_str(), "wb");
  Oracle.sound_fp[1] = fopen((d + "/samples.bin").c_str(), "wb");
  SOUND_OracleWrite = Oracle_SoundWrite;
  SOUND_OracleSample = Oracle_SoundSample;
 }
}

static void Oracle_WriteWords(const std::string& path, const uint16* words, size_t count)
{
 FILE* fp = fopen(path.c_str(), "wb");

 if(!fp)
  return;
 for(size_t i = 0; i < count; i++)
 {
  fputc(words[i] >> 8, fp);
  fputc(words[i] & 0xFF, fp);
 }
 fclose(fp);
}

static void Oracle_WriteScreen(const std::string& path, const EmulateSpecStruct* es)
{
 const MDFN_Surface* s = es->surface;
 const MDFN_Rect& r = es->DisplayRect;
 int32 w = r.w;
 FILE* fp;

 if(es->LineWidths)	// per-line widths; in an interlaced frame only the current field's lines have one
 {
  for(int32 y = r.y; y < r.y + r.h; y++)
   w = std::max<int32>(w, es->LineWidths[y]);
 }
 if(!s || w <= 0 || r.h <= 0 || !(fp = fopen(path.c_str(), "wb")))
  return;
 fprintf(fp, "P6\n%d %d\n255\n", (int)w, (int)r.h);
 for(int32 y = r.y; y < r.y + r.h; y++)
 {
  const uint32* row = s->pixels + (size_t)y * s->pitchinpix + r.x;

  for(int32 x = 0; x < w; x++)
  {
   int cr, cg, cb;

   s->DecodeColor(row[x], cr, cg, cb);
   fputc(cr, fp);
   fputc(cg, fp);
   fputc(cb, fp);
  }
 }
 fclose(fp);
}

static void Oracle_DumpTo(const std::string& d, const EmulateSpecStruct* es)
{
 mkdir(d.c_str(), 0755);
 Oracle_WriteWords(d + "/wram-l.bin", WorkRAML, sizeof(WorkRAML) / sizeof(WorkRAML[0]));
 Oracle_WriteWords(d + "/wram-h.bin", WorkRAMH, sizeof(WorkRAMH) / sizeof(WorkRAMH[0]));
 Oracle_WriteWords(d + "/vdp1-vram.bin", VDP1::VRAM, 0x40000);
 Oracle_WriteWords(d + "/vdp2-vram.bin", VDP2::OracleVRAM(), 0x40000);
 Oracle_WriteWords(d + "/vdp2-cram.bin", VDP2::OracleCRAM(), 0x800);
 Oracle_WriteWords(d + "/sound-ram.bin", SOUND_OracleRAM(), 0x40000);
 {
  uint16 eeprom[0x40];

  STVIO_OracleEEPROM(eeprom);
  Oracle_WriteWords(d + "/eeprom.bin", eeprom, 0x40);
 }
 if(FILE* fp = fopen((d + "/regs.txt").c_str(), "w"))
 {
  for(unsigned c = 0; c < 2; c++)
  {
   fprintf(fp, "%s PC=%08x PR=%08x SR=%08x GBR=%08x VBR=%08x MACH=%08x MACL=%08x\n", c ? "slave " : "master",
	CPU[c].PC, CPU[c].PR, CPU[c].SR, CPU[c].GBR, CPU[c].VBR, CPU[c].MACH, CPU[c].MACL);
   for(unsigned i = 0; i < 16; i++)
    fprintf(fp, "%sR%u=%08x", (i % 8) ? " " : "  ", i, CPU[c].R[i]);
   fprintf(fp, "\n");
  }
  fprintf(fp, "skip=%d rect=%d,%d,%d,%d linewidth0=%d interlaced=%d field=%d\n", (int)es->skip,
	(int)es->DisplayRect.x, (int)es->DisplayRect.y, (int)es->DisplayRect.w, (int)es->DisplayRect.h,
	es->LineWidths ? (int)es->LineWidths[es->DisplayRect.y] : -1, (int)es->InterlaceOn, (int)es->InterlaceField);
  fclose(fp);
 }
 if(FILE* fp = fopen((d + "/regs.bin").c_str(), "wb"))
 {
  uint32 sound[19];

  for(unsigned c = 0; c < 2; c++)
  {
   const uint32 regs[23] = { CPU[c].R[0], CPU[c].R[1], CPU[c].R[2], CPU[c].R[3], CPU[c].R[4], CPU[c].R[5],
	CPU[c].R[6], CPU[c].R[7], CPU[c].R[8], CPU[c].R[9], CPU[c].R[10], CPU[c].R[11], CPU[c].R[12],
	CPU[c].R[13], CPU[c].R[14], CPU[c].R[15], CPU[c].PC, CPU[c].PR, CPU[c].SR, CPU[c].GBR, CPU[c].VBR,
	CPU[c].MACH, CPU[c].MACL };

   for(uint32 v : regs)
   {
    const uint8 b[4] = { (uint8)(v >> 24), (uint8)(v >> 16), (uint8)(v >> 8), (uint8)v };

    fwrite(b, 1, 4, fp);
   }
  }
  SOUND_OracleRegs(sound);
  for(uint32 v : sound)
  {
   const uint8 b[4] = { (uint8)(v >> 24), (uint8)(v >> 16), (uint8)(v >> 8), (uint8)v };

   fwrite(b, 1, 4, fp);
  }
  fclose(fp);
 }
 Oracle_WriteScreen(d + "/screen.ppm", es);
}

static std::string Oracle_FrameDir(char kind, uint64 frame)
{
 char name[32];

 snprintf(name, sizeof(name), "/%c%06llu", kind, (unsigned long long)frame);
 return Oracle.dir + name;
}

static void Oracle_TraceStart(const EmulateSpecStruct* es)
{
 const std::string d = Oracle_FrameDir('t', Oracle.frame + 1);

 mkdir(d.c_str(), 0755);
 Oracle_DumpTo(d + "/start", es);
 static const char* const names[3] = { "/m.trace", "/s.trace", "/k.trace" };

 for(unsigned c = 0; c < 3; c++)
 {
  Oracle.trace_fp[c] = fopen((d + names[c]).c_str(), "wb");
  Oracle.trace_buf[c].clear();
 }
 Oracle.tracing = true;
 M68K::OracleHook = Oracle_SoundHook;
 for(unsigned c = 0; c < 3; c++)
  Oracle_Record(c, 'H', 3, 0x53545654, 0);
}

static void Oracle_TraceStop(void)
{
 Oracle.tracing = false;
 M68K::OracleHook = nullptr;
 for(unsigned c = 0; c < 3; c++)
 {
  fwrite(Oracle.trace_buf[c].data(), 1, Oracle.trace_buf[c].size(), Oracle.trace_fp[c]);
  Oracle.trace_buf[c].clear();
  fclose(Oracle.trace_fp[c]);
  Oracle.trace_fp[c] = nullptr;
 }
}

static uint64 Oracle_Hash(const uint16* words, size_t count)
{
 uint64 h = 0xcbf29ce484222325ULL;

 for(size_t i = 0; i < count; i++)
 {
  h = (h ^ (words[i] >> 8)) * 0x100000001b3ULL;
  h = (h ^ (words[i] & 0xFF)) * 0x100000001b3ULL;
 }
 return h;
}

static void Oracle_WriteHashes(void)
{
 fprintf(Oracle.hashes_fp, "%llu %016llx %016llx %016llx %016llx %016llx %016llx %016llx %08x %08x\n",
	(unsigned long long)Oracle.frame,
	(unsigned long long)Oracle_Hash(WorkRAML, sizeof(WorkRAML) / sizeof(WorkRAML[0])),
	(unsigned long long)Oracle_Hash(WorkRAMH, sizeof(WorkRAMH) / sizeof(WorkRAMH[0])),
	(unsigned long long)Oracle_Hash(VDP1::VRAM, 0x40000),
	(unsigned long long)Oracle_Hash(VDP2::OracleVRAM(), 0x40000),
	(unsigned long long)Oracle_Hash(VDP2::OracleCRAM(), 0x800),
	(unsigned long long)Oracle_Hash(VDP2::OracleRegs(), 0x100),
	(unsigned long long)Oracle_Hash(SOUND_OracleRAM(), 0x40000),
	CPU[0].PC, CPU[1].PC);
}

// A VDP1 draw starting or ending in a frame to be dumped.
static void Oracle_Draw(bool end)
{
 const std::string d = Oracle_FrameDir('f', Oracle.frame + 1);
 const std::string dd = d + "/draw";

 Oracle.draw_started = !end;
 if(!end)
 {
  uint16 regs[6];

  mkdir(d.c_str(), 0755);
  mkdir(dd.c_str(), 0755);
  Oracle_WriteWords(dd + "/vdp1-vram.bin", VDP1::VRAM, 0x40000);
  Oracle_WriteWords(dd + "/fb-before.bin", VDP1::FB[!VDP1::OracleDisplayFB()], 0x20000);
  VDP1::OracleRegs(regs);
  Oracle_WriteWords(dd + "/vdp1-regs.bin", regs, 6);
 }
 else
 {
  Oracle_WriteWords(dd + "/fb-after.bin", VDP1::FB[!VDP1::OracleDisplayFB()], 0x20000);
  VDP1::OracleDrawHook = nullptr;
 }
}

// At VBlank-OUT of a frame to be dumped: the state its picture is drawn from.
static void Oracle_VBlankOut(void)
{
 const std::string d = Oracle_FrameDir('f', Oracle.frame + 1);
 const std::string dd = d + "/display";

 VDP2::OracleVBlankOutHook = nullptr;
 mkdir(d.c_str(), 0755);
 mkdir(dd.c_str(), 0755);
 Oracle_WriteWords(dd + "/vdp2-regs.bin", VDP2::OracleRegs(), 0x100);
 Oracle_WriteWords(dd + "/vdp2-vram.bin", VDP2::OracleVRAM(), 0x40000);
 Oracle_WriteWords(dd + "/vdp2-cram.bin", VDP2::OracleCRAM(), 0x800);
 // the framebuffer VDP1 has just drawn: the frame change at this VBlank-OUT puts it on display
 Oracle_WriteWords(dd + "/vdp1-fb.bin", VDP1::FB[!VDP1::OracleDisplayFB()], 0x20000);
 if(FILE* fp = fopen((dd + "/vdp1-regs.bin").c_str(), "wb"))
 {
  fputc(VDP1::TVMR, fp);
  fputc(VDP1::FBCR, fp);
  fclose(fp);
 }
}

static void Oracle_BeginFrame(const EmulateSpecStruct* es)
{
 if(Oracle.on)
 {
  for(uint64 f : Oracle.dumps)
  {
   if(f == Oracle.frame + 1)
   {
    VDP2::OracleVBlankOutHook = Oracle_VBlankOut;
    VDP1::OracleDrawHook = Oracle_Draw;
   }
  }
 }
 if(Oracle.on && Oracle.frame == 0 && !Oracle.tracing && Oracle.trace_next < Oracle.traces.size() && Oracle.traces[Oracle.trace_next].first == 1)
 {	// a window from power on
  Oracle.trace_end = Oracle.traces[Oracle.trace_next].second;
  Oracle.trace_next++;
  Oracle_TraceStart(es);
 }
}

static void Oracle_EndFrame(const EmulateSpecStruct* es)
{
 if(!Oracle.on)
  return;
 // the frame's hooks are its own: a VBlank-OUT that did not come, a draw that did not start
 VDP2::OracleVBlankOutHook = nullptr;
 if(!Oracle.draw_started)
  VDP1::OracleDrawHook = nullptr;
 Oracle.frame++;
 if(Oracle.hashes_fp)
  Oracle_WriteHashes();
 if(Oracle.sound_end)
 {
  Oracle_SoundRecord(SOUND_OracleSamples, 'F', 0, (uint32)Oracle.frame, 0);
  if(Oracle.frame >= Oracle.sound_end)
  {
   SOUND_OracleWrite = nullptr;
   SOUND_OracleSample = nullptr;
   fclose(Oracle.sound_fp[0]);
   fclose(Oracle.sound_fp[1]);
   Oracle.sound_end = 0;
  }
 }
 if(Oracle.tracing)
 {
  for(unsigned c = 0; c < 3; c++)
   Oracle_Record(c, 'F', 0, (uint32)Oracle.frame, 0);
  if(Oracle.frame >= Oracle.trace_end)
   Oracle_TraceStop();
 }
 for(uint64 f : Oracle.dumps)
 {
  if(f == Oracle.frame)
   Oracle_DumpTo(Oracle_FrameDir('f', Oracle.frame), es);
 }
 if(!Oracle.tracing && Oracle.trace_next < Oracle.traces.size() && Oracle.frame + 1 == Oracle.traces[Oracle.trace_next].first)
 {	// the next frame is the first of a window
  Oracle.trace_end = Oracle.frame + Oracle.traces[Oracle.trace_next].second;
  Oracle.trace_next++;
  Oracle_TraceStart(es);
 }
 if(Oracle.exit_frame && Oracle.frame >= Oracle.exit_frame)
 {
  if(Oracle.tracing)
   Oracle_TraceStop();
  if(Oracle.hashes_fp)
   fclose(Oracle.hashes_fp);
  fflush(stdout);
  std::exit(0);
 }
}
