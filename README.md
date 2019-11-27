# Stardict

Rust implementation of Webserver for StarDict dictionary.

## .dz file can be extracted by gzip

```bash
gzip -cd XYZ.dict.dz > XYZ.dict
```

## Usage
just run the stardict command. many of the configurations in `res/` follow my C++ version [sdwv](https://github.com/tomgrean/sdwv/) except using @ for variable replace and @p for dictionary path.

the program default uses `/usr/share/stardict/dic/` as dictionary directory.
copy everything in `res/` to dictionary directory, eg: `cp -r res/* /usr/share/stardict/dic/`
and then start the command with `./stardict`.
Open a browser and access `http://localhost:8888` or replace _localhost_ with an exact IP address.

## Edit dict

Beside stardict tools on the net, you can try dict extract and gendict from [mytool](https://github.com/tomgrean/tools).

## Documents

-   [Format for StarDict dictionary files](https://github.com/huzheng001/stardict-3/blob/master/dict/doc/StarDictFileFormat)

-   [StarDictFileFormat](https://github.com/huzheng001/stardict-3/blob/master/dict/doc/StarDictFileFormat)

-   [Notes about stardict dictionry format](http://dhyannataraj.github.io/blog/2010/10/04/Notes-about-stardict-dictionry-format/)

-   [stardict](http://kdr2.com/resource/stardict.html)

