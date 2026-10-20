/* stv-recomp oracle hooks for Mednafen's Saturn/ST-V core (included at the end of src/ss/ss.cpp).
 *
 * A development tool only: Mednafen is GPL-2+, and nothing here is shipped or linked into the port.
 * Driven by environment variables so the stock command line still works:
 *
 *   STV_ORACLE_DIR   directory to write into (the hooks are off when it is unset)
 *   STV_ORACLE_DUMP  frames to dump at, comma separated ("600,1200")
 *   STV_ORACLE_EXIT  exit after this frame (after its dump, if any)
 *
 * Frames count Emulate() calls from 1. A dump at frame F is DIR/fFFFFFF/:
 *   wram-l.bin wram-h.bin      work RAM, 1 MB each, big-endian bytes as the SH-2 sees them
 *   vdp1-vram.bin              VDP1 VRAM (512 KB)
 *   vdp2-vram.bin vdp2-cram.bin
 *   sound-ram.bin              SCSP RAM (512 KB)
 *   regs.txt                   both SH-2s' registers
 *   screen.ppm                 the frame as displayed (DisplayRect, the widest line's width)
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
} Oracle;

static void Oracle_Init(void)
{
 const char* dir = getenv("STV_ORACLE_DIR");

 Oracle.on = (dir && *dir);
 if(!Oracle.on)
  return;
 Oracle.dir = dir;
 Oracle.frame = 0;
 Oracle.exit_frame = 0;
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

static void Oracle_Dump(const EmulateSpecStruct* es)
{
 char name[32];

 snprintf(name, sizeof(name), "/f%06llu", (unsigned long long)Oracle.frame);
 const std::string d = Oracle.dir + name;

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
 Oracle_WriteScreen(d + "/screen.ppm", es);
}

static void Oracle_EndFrame(const EmulateSpecStruct* es)
{
 if(!Oracle.on)
  return;
 Oracle.frame++;
 for(uint64 f : Oracle.dumps)
 {
  if(f == Oracle.frame)
   Oracle_Dump(es);
 }
 if(Oracle.exit_frame && Oracle.frame >= Oracle.exit_frame)
 {
  fflush(stdout);
  std::exit(0);
 }
}
