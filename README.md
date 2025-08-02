# MusicQuiz
A client / server application which satisfies your need for music appreciation!

Some of the exiting new features are:
 - A rust code base which is mostly free of `unwrap()` statements and decent error handling
 - A new reveal feature on the client which shows the result from the previous song
 - A clipboard feature allowing for copy-pasting the server url (yes, only the server url for now)
 - Automatic binary building and distribution on GitHub via releases (for now only windows binaries are built)

Some notably missing features include:
 - Ability to select audio output device on startup
 - Preview of how many people already correctly stated the song title / interpret
 - Server configuration via config file / env variables
 - Major rework of the code base, splitting into multiple (shared) modules

As you can see while there has been progress, there are already plenty ideas of how the music quiz experience could be improved, so feel free to contribute!

## Road to V0.1
Until the first major release, I have compiled a list of changes that I consider necessary for that:

 - [x] Removal of most unwrap statements in server and client code
 - [x] Robust logging mechanism (somewhat)
 - [ ] Server configuration using some appropriate file format
 - [x] Save grading history of a client to file
 - [ ] General bug detection and removal
 - [x] CI tasks and automatic build config for releases
 - [x] Client clipboard support for server url