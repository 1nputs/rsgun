/* The oracle's SCSP on its own: replays a sound log's writes and resets (tools/oracle/run.py --sound,
 * or a crafted probe in the same format) and writes the samples it makes.
 *
 *   scsp_oracle WRITES.bin OUT.bin [SAMPLES [RAM.bin]]
 *
 * OUT.bin: two big-endian 16-bit words a sample, left and right, as the log's samples.bin. SAMPLES
 * defaults to the log's last stamp. Before making sample n, every record stamped n is applied.
 * RAM.bin: sound RAM at the end (big-endian), for the DSP's ring buffer. With SCSP_ORACLE_REGS=N:PATH
 * in the environment, the registers as the CPUs read them (0x000-0xFFF, big-endian) after sample N.
 *
 * A development tool only: it compiles Mednafen's SCSP (GPL-2.0-or-later) in, like oracle.inc, and is
 * never shipped or linked into the port. Built by tools/scsp/build.sh into build/scsp_oracle.
 */
#include <mednafen/mednafen.h>
#include <mednafen/state.h>
#include <stdio.h>
#include <string.h>
#include <string>
#include <vector>

using namespace Mednafen;

// save states are not used here
namespace Mednafen
{
 bool MDFNSS_StateAction(StateMem* sm, const unsigned load, const bool data_only, const SFORMAT* sf, const char* sname, const bool optional) noexcept
 {
  return false;
 }
}

namespace MDFN_IEN_SS
{
 enum
 {
  SS_DBG_ERROR = (1U << 0),
  SS_DBG_WARNING = (1U << 1),
  SS_DBG_M68K = (1U << 2),
  SS_DBG_SCSP = (1U << 26),
  SS_DBG_SCSP_REGW = (1U << 27),
  SS_DBG_SCSP_MOBUF = (1U << 28),
 };
 enum : uint32 { ss_dbg_mask = 0 };
 static INLINE void SS_DBG(uint32 which, const char* format, ...) { }
 template<unsigned which>
 static void SS_DBG_Wrap(const char* format, ...) noexcept { }

#include "ss/scsp.h"

 static unsigned SoundLevel;
 static bool MainInt;
 static INLINE void SCSP_SoundIntChanged(SS_SCSP* s, unsigned level) { SoundLevel = level; }
 static INLINE void SCSP_MainIntChanged(SS_SCSP* s, bool state) { MainInt = state; }

#include "ss/scsp.inc"
}

using namespace MDFN_IEN_SS;

static SS_SCSP SCSP;

static uint32 be32(const uint8* p)
{
 return ((uint32)p[0] << 24) | ((uint32)p[1] << 16) | ((uint32)p[2] << 8) | p[3];
}

int main(int argc, char** argv)
{
 if(argc < 3)
 {
  fprintf(stderr, "usage: scsp_oracle WRITES.bin OUT.bin [SAMPLES]\n");
  return 2;
 }
 FILE* in = fopen(argv[1], "rb");
 if(!in)
 {
  perror(argv[1]);
  return 2;
 }
 std::vector<uint8> log;
 uint8 buf[1 << 16];
 size_t n;
 while((n = fread(buf, 1, sizeof(buf), in)) > 0)
  log.insert(log.end(), buf, buf + n);
 fclose(in);
 const size_t records = log.size() / 16;
 uint64 total = records ? be32(&log[(records - 1) * 16]) : 0;
 if(argc > 3)
  total = strtoull(argv[3], nullptr, 10);
 FILE* out = fopen(argv[2], "wb");
 if(!out)
 {
  perror(argv[2]);
  return 2;
 }
 SCSP.Reset(true);
 uint64 regs_at = ~(uint64)0;
 std::string regs_path;
 if(const char* e = getenv("SCSP_ORACLE_REGS"))
 {
  const char* colon = strchr(e, ':');
  if(colon)
  {
   regs_at = strtoull(e, nullptr, 10);
   regs_path = colon + 1;
  }
 }
 size_t r = 0;
 std::vector<uint8> obuf;
 for(uint64 s = 0; s < total; s++)
 {
  for(; r < records && be32(&log[r * 16]) <= s; r++)
  {
   const uint8* rec = &log[r * 16];
   const uint8 tag = rec[4], size = rec[5];
   const uint32 a = be32(rec + 8), v = be32(rec + 12);

   if(tag == 'R')
    SCSP.Reset(a != 0);
   else if(tag == 'W' && size == 1)
   {
    uint8 b = v;
    SCSP.RW<uint8, true>(a & 0x1FFFFF, b);
   }
   else if(tag == 'W' && size == 2)
   {
    uint16 w = v;
    SCSP.RW<uint16, true>(a & 0x1FFFFF, w);
   }
  }
  int16 lr[2];
  SCSP.RunSample(lr);
  if(s == regs_at)
  {
   FILE* rf = fopen(regs_path.c_str(), "wb");
   for(uint32 a = 0; a < 0x1000 && rf; a += 2)
   {
    uint16 v;
    SCSP.RW<uint16, false>(0x100000 | a, v);
    fputc(v >> 8, rf);
    fputc(v & 0xFF, rf);
   }
   if(rf)
    fclose(rf);
  }
  const uint8 o[4] = { (uint8)((uint16)lr[0] >> 8), (uint8)lr[0], (uint8)((uint16)lr[1] >> 8), (uint8)lr[1] };
  obuf.insert(obuf.end(), o, o + 4);
  if(obuf.size() >= (1U << 20))
  {
   fwrite(obuf.data(), 1, obuf.size(), out);
   obuf.clear();
  }
 }
 fwrite(obuf.data(), 1, obuf.size(), out);
 fclose(out);
 if(argc > 4)
 {
  FILE* rf = fopen(argv[4], "wb");
  if(!rf)
  {
   perror(argv[4]);
   return 2;
  }
  const uint16* ram = SCSP.GetRAMPtr();
  for(unsigned i = 0; i < 0x40000; i++)
  {
   fputc(ram[i] >> 8, rf);
   fputc(ram[i] & 0xFF, rf);
  }
  fclose(rf);
 }
 return 0;
}
