QoS Core v0.1.0 — shared core of the QoS suite.
Author: Lewis Cameron. For Captain of Industry v0.8.8 (Update 4.3).

WHAT IT IS
The foundation sub-mod for a suite of small quality-of-life mods.
On its own it changes NO gameplay: it provides the suite's shared
logging, a registry that feature mods announce themselves to, and
the suite's config conventions. Feature mods (QoSChickens today;
more to come) declare a dependency on QoSCore in their manifests
and will not load without it — install Core first, or just use the
combined QoS suite pack, which contains every suite folder.

INSTALL
1. Close the game completely.
2. Unzip so the QoSCore folder lands in:
      %APPDATA%\Captain of Industry\Mods\
   Final layout:
      %APPDATA%\Captain of Industry\Mods\QoSCore\manifest.json
      %APPDATA%\Captain of Industry\Mods\QoSCore\QoSCore.dll
      %APPDATA%\Captain of Industry\Mods\QoSCore\config.json
3. Launch the game; enable mods in Settings -> Miscellaneous if
   not already enabled, then load your save and tick QoSCore in
   the save's mod list if asked.

CONFIG (Settings -> the mod's options, persisted per save)
  verbose_logging (default: false)
    Also log diagnostic detail (per-pass decisions and skip
    reasons) from every QoS feature, not just actions taken.

REMOVAL SAFETY
QoSCore registers no prototypes, recipes or entities and stores
nothing in your save beyond its own config values (inert once the
mod is gone). Delete the QoSCore folder to remove it. Note:
feature mods that depend on QoSCore will then refuse to load —
the game reports the missing dependency; your save is not harmed.
Remove or update the feature mods first if you remove Core.
