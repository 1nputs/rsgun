/* stv-recomp oracle hooks for Mednafen's Saturn/ST-V core (included in src/ss/ss.cpp before debug.inc).
 *
 * A development tool only: Mednafen is GPL-2+, and nothing here is shipped or linked into the port.
 * Driven by environment variables so the stock command line still works:
 *
 *   STV_ORACLE_DIR    directory to write into (the hooks are off when it is unset)
 *   STV_ORACLE_DUMP   frames to dump at, comma separated ("600,1200")
 *   STV_ORACLE_EXIT   exit after this frame (after its dump and trace, if any)
 *   STV_ORACLE_TRACE  F:N[,F:N...]  trace both SH-2s through frames F to F+N-1 (F >= 2; windows in order)
 *
 * Frames count Emulate() calls from 1. A dump at frame F (taken after the frame) is DIR/fFFFFFF/:
 *   wram-l.bin wram-h.bin      work RAM, 1 MB each, big-endian bytes as the SH-2 sees them
 *   vdp1-vram.bin              VDP1 VRAM (512 KB)
 *   vdp2-vram.bin vdp2-cram.bin
 *   sound-ram.bin              SCSP RAM (512 KB)
 *   regs.txt                   both SH-2s' registers, and the frame's geometry
 *   regs.bin                   the same registers: per CPU (master, slave) 23 big-endian words:
 *                              R0-R15, PC, PR, SR, GBR, VBR, MACH, MACL
 *   screen.ppm                 the frame as displayed (DisplayRect, the widest line's width)
 *
 * A trace from frame F is DIR/tFFFFFF/: start/ (a dump taken before frame F, the state the trace
 * starts from) and m.trace, s.trace (the master's and the slave's records, in the order the oracle
 * made them). Version 3 records are 12 bytes, big-endian:
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
 * The file begins with one header record: 'H', size 3 (the version), a = 0x53545654 ("STVT"), v = 0.
 */

#include <sys/stat.h>

namespace VDP1 { MDFN_HIDE extern uint16 VRAM[0x40000]; }	// (declared only inside an inline in vdp1.h)

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
 FILE* trace_fp[2];
 std::vector<uint8> trace_buf[2];
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

static void Oracle_Init(void)
{
 const char* dir = getenv("STV_ORACLE_DIR");

 Oracle.on = (dir && *dir);
 Oracle.tracing = false;
 if(!Oracle.on)
  return;
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
 for(unsigned c = 0; c < 2; c++)
 {
  Oracle.trace_fp[c] = fopen((d + (c ? "/s.trace" : "/m.trace")).c_str(), "wb");
  Oracle.trace_buf[c].clear();
 }
 Oracle.tracing = true;
 for(unsigned c = 0; c < 2; c++)
  Oracle_Record(c, 'H', 3, 0x53545654, 0);
}

static void Oracle_TraceStop(void)
{
 Oracle.tracing = false;
 for(unsigned c = 0; c < 2; c++)
 {
  fwrite(Oracle.trace_buf[c].data(), 1, Oracle.trace_buf[c].size(), Oracle.trace_fp[c]);
  Oracle.trace_buf[c].clear();
  fclose(Oracle.trace_fp[c]);
  Oracle.trace_fp[c] = nullptr;
 }
}

static void Oracle_EndFrame(const EmulateSpecStruct* es)
{
 if(!Oracle.on)
  return;
 Oracle.frame++;
 if(Oracle.tracing)
 {
  for(unsigned c = 0; c < 2; c++)
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
  fflush(stdout);
  std::exit(0);
 }
}
