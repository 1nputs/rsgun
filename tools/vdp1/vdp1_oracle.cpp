/* The oracle's VDP1 on its own: draws a command list from a VRAM image into a framebuffer, as the
 * oracle does, and writes the framebuffer after the draw.
 *
 *   vdp1_oracle DRAW_DIR
 *
 * DRAW_DIR holds a draw as the oracle dumps one (tools/patches/mednafen/oracle.inc, draw/):
 * vdp1-vram.bin (512 KB), fb-before.bin (256 KB, the framebuffer drawn into), vdp1-regs.bin (TVMR,
 * FBCR, PTMR, EWDR, EWLR, EWRR, big-endian words). Writes DRAW_DIR/fb-oracle.bin.
 *
 * A development tool only: it links Mednafen's VDP1 (GPL-2.0-or-later) from the patched tree in ref/
 * (tools/fetch_refs.sh builds it), like oracle.inc, and is never shipped or linked into the port.
 * Built by tools/vdp1/build.sh into build/vdp1_oracle.
 */
#include <mednafen/mednafen.h>
#include <stdio.h>
#include <string>
#include <vector>
#include "ss/ss.h"
#include "ss/vdp1.h"

using namespace Mednafen;
using namespace MDFN_IEN_SS;

namespace MDFN_IEN_SS
{
 namespace VDP1
 {
  extern uint16 VRAM[0x40000];
  extern uint16* FBDrawWhichPtr;
 }
}

static bool done;
static void DrawHook(bool end)
{
 if(end)
  done = true;
}

static std::vector<uint8> ReadFile(const std::string& path)
{
 std::vector<uint8> d;
 FILE* fp = fopen(path.c_str(), "rb");
 if(!fp)
  return d;
 uint8 buf[65536];
 size_t n;
 while((n = fread(buf, 1, sizeof(buf), fp)) > 0)
  d.insert(d.end(), buf, buf + n);
 fclose(fp);
 return d;
}

int main(int argc, char** argv)
{
 if(argc < 2)
 {
  fprintf(stderr, "usage: vdp1_oracle DRAW_DIR\n");
  return 2;
 }
 const std::string dir = argv[1];
 const std::vector<uint8> vram = ReadFile(dir + "/vdp1-vram.bin"), fb = ReadFile(dir + "/fb-before.bin"), regs = ReadFile(dir + "/vdp1-regs.bin");
 if(vram.size() != 0x80000 || fb.size() != 0x40000 || regs.size() < 12)
 {
  fprintf(stderr, "vdp1_oracle: %s: needs vdp1-vram.bin, fb-before.bin, vdp1-regs.bin\n", dir.c_str());
  return 2;
 }
 auto reg = [&](unsigned i) { return (uint16)((regs[2 * i] << 8) | regs[2 * i + 1]); };
 VDP1::Init();
 VDP1::Reset(true);
 VDP1::Write16_DB(0x100000, reg(0));	// TVMR
 VDP1::Write16_DB(0x100002, reg(1));	// FBCR
 VDP1::Write16_DB(0x100006, reg(3));	// EWDR
 VDP1::Write16_DB(0x100008, reg(4));	// EWLR
 VDP1::Write16_DB(0x10000A, reg(5));	// EWRR
 for(unsigned i = 0; i < 0x40000; i++)
  VDP1::VRAM[i] = (vram[2 * i] << 8) | vram[2 * i + 1];
 for(unsigned i = 0; i < 0x20000; i++)
  VDP1::FBDrawWhichPtr[i] = (fb[2 * i] << 8) | fb[2 * i + 1];
 VDP1::OracleDrawHook = DrawHook;
 VDP1::Write16_DB(0x100004, 0x0001);	// PTMR: draw now
 sscpu_timestamp_t ts = 0;
 for(unsigned i = 0; i < 100000000 && !done; i++)
 {
  ts += 1000;
  VDP1::Update(ts);
  if(ts > 0x30000000)
   ts = 0, VDP1::AdjustTS(-0x30000000);
 }
 if(!done)
 {
  fprintf(stderr, "vdp1_oracle: the draw did not end\n");
  return 1;
 }
 FILE* out = fopen((dir + "/fb-oracle.bin").c_str(), "wb");
 for(unsigned i = 0; out && i < 0x20000; i++)
 {
  fputc(VDP1::FBDrawWhichPtr[i] >> 8, out);
  fputc(VDP1::FBDrawWhichPtr[i] & 0xFF, out);
 }
 if(out)
  fclose(out);
 return 0;
}
