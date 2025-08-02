# Music Quiz Release
First preliminary release :clap:! While not all the open points from the roadmap have yet been addressed, the majority and has seen significant work to the point
where the current version should be suitable for usage in the 2025 official music quiz!

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