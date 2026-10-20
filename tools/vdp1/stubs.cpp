/* Stand-ins for the rest of the oracle's machine, for vdp1_oracle.cpp (a development tool). */
#include <mednafen/mednafen.h>
#include <mednafen/state.h>
#include "ss/ss.h"

using namespace Mednafen;

namespace Mednafen
{
 bool MDFNSS_StateAction(StateMem* sm, const unsigned load, const bool data_only, const SFORMAT* sf, const char* sname, const bool optional) noexcept
 {
  return false;
 }
}

namespace MDFN_IEN_SS
{
 uint32 ss_horrible_hacks = 0;
 int32 SH7095_mem_timestamp = 0;
 event_list_entry events[SS_EVENT__COUNT];
 void SS_SetEventNT(event_list_entry* e, const sscpu_timestamp_t next_timestamp) { }
 void SS_SetPhysMemMap(uint32 Astart, uint32 Aend, uint16* ptr, uint32 length, bool is_writeable) { }
 void SCU_SetInt(unsigned which, bool active) { }
 bool SCU_CheckVDP1HaltKludge(void) { return false; }
 namespace VDP2
 {
  sscpu_timestamp_t Update(sscpu_timestamp_t timestamp) { return timestamp + 1000000; }
 }
}

namespace Mednafen
{
 void MDFND_OutputNotice(MDFN_NoticeType t, const char* s) noexcept { }
}
