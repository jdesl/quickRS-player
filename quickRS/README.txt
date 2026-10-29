quickRS 0.3.0
=============

A practice player for your own Rocksmith 2014 songs, played with your real guitar: scrolling tab or a 3D
highway, loops, slow-down, pitch shift, note by note, takes, a pedalboard and an amp room.
Your notes are scored as you play, and R records what you hear.

You need: Windows 10 or 11 (64-bit), your guitar (a Real Tone Cable, an audio interface, or a mic), and your own
Rocksmith 2014 songs. quickRS comes with no songs and no Rocksmith keys.

There's no installer: this folder, the one with quickrs.exe in it, is quickRS ("the quickRS folder"). Keep it
somewhere you can write to (Documents, Desktop, another drive), not in C:\Program Files, so it can update itself.

The full guide, with pictures, is at https://github.com/arpan2711/quickRS-player (the docs folder there).
The Mac version (beta) is on the same page: see docs/INSTALL.md.


1. Start quickRS
----------------
  1. Double-click quickrs.exe.
  2. Windows may warn about an unknown app, because the programs aren't signed. Click "More info", then
     "Run anyway".
  3. A short welcome shows three steps: your song folder, the keys, your guitar input. quickRS has already
     looked for Rocksmith's dlc folder in your Steam libraries. Got it (Esc) closes the welcome; Settings (F2) >
     Directories > Welcome guide opens it again.
  4. A small card then offers "Set up your guitar?" once: it walks you through your input, output, level and
     calibration in a couple of minutes (see section 4).

For a Desktop shortcut: right-click make_shortcut.ps1 and choose "Run with PowerShell".


2. Your songs
-------------
quickRS plays your .psarc files straight from Rocksmith's dlc folder (with Steam it's usually
C:\Program Files (x86)\Steam\steamapps\common\Rocksmith2014\dlc), and feedpaks, an open one-file-per-song format.
You don't need to convert anything.

Settings (F2) > Directories > SONG FOLDERS lists the folders quickRS reads. "Add folder..." adds one (or drag a
folder onto the window), "Remove" takes one out, "Rescan folders" (F5) reads them again.
"Search Rocksmith's dlc folder" can be turned off if you only want your feedpaks.
Each song is marked feedpak, psarc, or feedpak + psarc.


3. Rocksmith's keys (only for .psarc files)
-------------------------------------------
.psarc files are encrypted. quickRS needs two keys to read them: the "PSARC key" and the "SNG key for PC". Each
is 64 characters of 0-9 and A-F, the same in every copy of Rocksmith. They are not a licence key and have nothing
to do with your Steam account. quickRS can't include them; they're published in open-source Rocksmith modding
tools. Feedpaks don't need them.

The easy way:
  1. Settings (F2) > Directories, scroll down to ROCKSMITH KEYS.
  2. Paste the PSARC key into "PSARC key" and the SNG key into "SNG key". (A whole keys.json text pasted into
     either box fills both.)
  3. Press "Save keys". Songs that said "needs keys" are read straight away: no restart, no F5.
"Load keys file..." uses a keys.json you already have.

Or make keys.json yourself: open keys.example.json in Notepad, replace each <...> (from < to >, both included)
with its key, and save it as keys.json in the quickRS folder. If Notepad saves it as keys.json.txt, that works
too. quickRS notices the file while it's open.

The ROCKSMITH KEYS card says whether keys were found and, if not, where it looked or what's wrong with the file.


4. Plug in your guitar
----------------------
The Real Tone Cable works as it does in Rocksmith: plug it in and quickRS picks it. Audio interfaces (WASAPI or
ASIO, including RS_ASIO setups), amp modellers and microphones work too. Settings (F2) > Audio input / output
shows what's picked. "Set up your guitar" there walks you through it step by step: the input, the output, a level
check, what's plugged in, and calibration.
Calibration works like Rocksmith's: guitar volume and tone knobs all the way up, hands off for 2 seconds, then
strum as hard as you can until the bar fills. If it says your interface clipped, turn the interface's gain down
and calibrate again. It recommends a trim and a pluck sensitivity, which you can adjust with sliders.


5. Convert to feedpaks (optional)
---------------------------------
"quickRS Converter.exe" turns your .psarc files into feedpaks, which play without keys. It needs the keys once,
to read the .psarc files, and never changes them. Its top line should say "Keys found: ready to convert". Leave
box 2 as it is (a feedpak folder next to dlc, which quickRS finds by itself) and press Convert. "Convert
songs.bat" does the same in a console window, and so does:
    quickrs-cli.exe convert-folder "C:\...\Rocksmith2014\dlc" "C:\...\Rocksmith2014\feedpak"


Updates
-------
quickRS looks for a new version at most once a day and offers it; nothing is downloaded until you press
"Update now". Settings (F2) > Updates turns the check off or checks now. An update replaces the program files
only: your keys, settings, scores and sounds stay.


Your data
---------
Settings, scores, favourites, playlists, saved licks, saved sounds, keys saved in Settings and your takes are
in %APPDATA%\quickRS\data (paste that into File Explorer's address bar). If a config.json or userdata.json is in
the quickRS folder (or in a "quickRS Data" folder inside it), that folder is used instead: a portable install.
Recordings (R) go to "quickRS Recordings" in your Music folder. Settings (F2) > Directories > RECORDINGS changes
where recordings and takes go.


Keyboard
--------
F2 settings, F11 full screen, Space play / pause, Esc back. Every button shows its key; in a song, F1 (or ?)
shows them all. R records what you hear (R again pauses, Shift+R stops and saves); Ctrl+R records a take of the
loop and checks it; G swaps the recording's guitar for the synth; V shows the RS Highway.


Command-line tools: quickrs-cli.exe --help (song info, tabs, audio, feedpak conversion, devices).
quickrs.exe --silent starts without any sound device.
Licences are in the licences folder: quickRS is MIT, the Manrope font is SIL OFL, and ww2ogg's codebooks are BSD.
quickRS isn't made by or connected with Ubisoft. Rocksmith is a trademark of Ubisoft Entertainment.
quickRS is free, but vibecoding isn't: https://ko-fi.com/arpanmitra
