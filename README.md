# Twitch Clip Downloader
A command line tool to download twitch clips using their new system.

## CLI Usage
<!-- CLI-DOCS-START -->

# Command-Line Help for `twdl`

This document contains the help content for the `twdl` command-line program.

**Command Overview:**

* [`twdl`↴](#twdl)
* [`twdl clip`↴](#twdl-clip)
* [`twdl channel`↴](#twdl-channel)

## `twdl`

Downloads twitch clips

**Usage:** `twdl <COMMAND>`

###### **Subcommands:**

* `clip` — 
* `channel` — 



## `twdl clip`

**Usage:** `twdl clip [OPTIONS] <CLIP>`

###### **Arguments:**

* `<CLIP>`

###### **Options:**

* `-o`, `--output <OUTPUT>` — Output dir to download clip to

  Default value: `.`
* `-L`, `--link` — Skip download and print the source file URL
* `-m`, `--metadata` — Download json metadata alongside the clip
* `-c`, `--credentials <CREDENTIALS>` — Path to a json file containing client_id and client_secret



## `twdl channel`

**Usage:** `twdl channel [OPTIONS] --credentials <CREDENTIALS>`

###### **Options:**

* `-o`, `--output <OUTPUT>` — Path to directory to store the clips

  Default value: `.`
* `-c`, `--credentials <CREDENTIALS>` — Path to a json file containing client_id and client_secret
* `-i`, `--broadcaster-id <BROADCASTER_ID>` — Numeric broadcaster ID
* `-l`, `--broadcaster-login <BROADCASTER_LOGIN>` — Broadcaster login
* `-s`, `--start <START_TIMESTAMP>` — Start of datetime range (If no end provided, defaults to 1 week)
* `-e`, `--end <END_TIMESTAMP>` — End of datetime range, requires a start time
* `-C`, `--chunk-size <CHUNK_SIZE>` — Number of clips fetched per page, default=20 max=100
* `-L`, `--link` — Skip downloads and print the source file URLs to stdout
* `-m`, `--metadata` — Download json metadata alongside the clip



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>


<!-- CLI-DOCS-END -->