# mediar

A command-line tool for organizing your media library using The Movie Database (TMDB) API. Automatically renames and organizes TV shows and movies into a structured directory layout with proper metadata.

![mediar demo](demo.svg)

## Features

- **Smart Media Detection**: Automatically identifies TV shows and movies from filenames
- **TV Show Organization**: Structures episodes by show name, season, and episode
- **Movie Organization**: Organizes movies with title and year
- **Multiple Operations**: Move, copy, or hard-link files
- **TMDB Integration**: Fetches accurate metadata from The Movie Database
- **Search Capability**: Search for TV shows and movies with filtering options
- **Safe Operations**: Preview changes before execution with confirmation prompts

## Installation

```bash
cargo install mediar
```

## Configuration

Set the TMDB API token environment variable ([Get one here](https://www.themoviedb.org/settings/api)):

```bash
export TMDB_API_TOKEN=your_tmdb_api_token_here
```

## Usage

### Search for Media

Search for TV shows and movies to find their TMDB IDs:

```shell
$ mediar search 'star trek generation'

╭─────┬────┬────────────────────────────────┬────┬──────┬──────┬──────────────────────────────────────╮
│ ID  │    │ Name                           │ 🌐 │ ⭐   │ Year │ TMDB Link                            │
├─────┼────┼────────────────────────────────┼────┼──────┼──────┼──────────────────────────────────────┤
│ 655 │ 📺 │ Star Trek: The Next Generation │ en │ 59.4 │ 1987 │ https://www.themoviedb.org/tv/655    │
│ 193 │ 🎬 │ Star Trek: Generations         │ en │ 2.9  │ 1994 │ https://www.themoviedb.org/movie/193 │
╰─────┴────┴────────────────────────────────┴────┴──────┴──────┴──────────────────────────────────────╯

Found 23 results (1 TV, 22 movies)
```

### Organize TV Shows

Link TV show episodes into a structured directory:

```bash
mediar link --tv-id tv_id --target /path/to/target /path/to/source
```

#### Example

```shell
$ tree
.
├── HandBrake Encodings
│   ├── Star.Trek.The.Next.Generation.S01E01.mkv
│   ├── Star.Trek.The.Next.Generation.S01E02.mkv
│   ├── Star.Trek.The.Next.Generation.S02E01.mkv
│   └── Star.Trek.The.Next.Generation.S02E02.mkv
└── Shows

3 directories, 4 files

$ mediar search 'star trek the next generation'

╭─────┬────┬────────────────────────────────┬────┬──────┬──────┬───────────────────────────────────╮
│ ID  │    │ Name                           │ 🌐 │ ⭐   │ Year │ TMDB Link                         │
├─────┼────┼────────────────────────────────┼────┼──────┼──────┼───────────────────────────────────┤
│ 655 │ 📺 │ Star Trek: The Next Generation │ en │ 54.9 │ 1987 │ https://www.themoviedb.org/tv/655 │
╰─────┴────┴────────────────────────────────┴────┴──────┴──────┴───────────────────────────────────╯

Found 19 results (1 TV, 18 movies)

$ mediar link --tv-id 655 -y HandBrake\ Encodings Shows/
Link: HandBrake Encodings/Star.Trek.The.Next.Generation.S01E01.mkv
↪ To: Shows/Star Trek The Next Generation (1987)/Season 01/Star Trek The Next Generation - S01E01 - Encounter at Farpoint.mkv
Link: HandBrake Encodings/Star.Trek.The.Next.Generation.S01E02.mkv
↪ To: Shows/Star Trek The Next Generation (1987)/Season 01/Star Trek The Next Generation - S01E02 - The Naked Now.mkv
Link: HandBrake Encodings/Star.Trek.The.Next.Generation.S02E01.mkv
↪ To: Shows/Star Trek The Next Generation (1987)/Season 02/Star Trek The Next Generation - S02E01 - The Child.mkv
Link: HandBrake Encodings/Star.Trek.The.Next.Generation.S02E02.mkv
↪ To: Shows/Star Trek The Next Generation (1987)/Season 02/Star Trek The Next Generation - S02E02 - Where Silence Has Lease.mkv
✓ Done.

$ tree
.
├── HandBrake Encodings
│   ├── Star.Trek.The.Next.Generation.S01E01.mkv
│   ├── Star.Trek.The.Next.Generation.S01E02.mkv
│   ├── Star.Trek.The.Next.Generation.S02E01.mkv
│   └── Star.Trek.The.Next.Generation.S02E02.mkv
└── Shows
    └── Star Trek The Next Generation (1987)
        ├── Season 01
        │   ├── Star Trek The Next Generation - S01E01 - Encounter at Farpoint.mkv
        │   └── Star Trek The Next Generation - S01E02 - The Naked Now.mkv
        └── Season 02
            ├── Star Trek The Next Generation - S02E01 - The Child.mkv
            └── Star Trek The Next Generation - S02E02 - Where Silence Has Lease.mkv

6 directories, 8 files
```

### Organize Movies

Organize movies by title and year:

```bash
# Interactive organization
mediar link --movie-id movie_id /path/to/source /path/to/target
```

#### Example

```shell
$ tree
.
├── HandBrake Encodings
│   └── Star.Trek.Generations.mkv
└── Movies

3 directories, 1 file

$ mediar search 'star trek generations'

╭─────┬────┬────────────────────────┬────┬─────┬──────┬──────────────────────────────────────╮
│ ID  │    │ Name                   │ 🌐 │ ⭐  │ Year │ TMDB Link                            │
├─────┼────┼────────────────────────┼────┼─────┼──────┼──────────────────────────────────────┤
│ 193 │ 🎬 │ Star Trek: Generations │ en │ 2.8 │ 1994 │ https://www.themoviedb.org/movie/193 │
╰─────┴────┴────────────────────────┴────┴─────┴──────┴──────────────────────────────────────╯

Found 2 results (0 TV, 2 movies)

$ mediar link --movie-id 193 -y HandBrake\ Encodings Movies/
Link: HandBrake Encodings/Star.Trek.Generations.mkv
↪ To: Movies/Star Trek Generations (1994)/Star Trek Generations (1994).mkv
✓ Done.

$ tree
.
├── HandBrake Encodings
│   └── Star.Trek.Generations.mkv
└── Movies
    └── Star Trek Generations (1994)
        └── Star Trek Generations (1994).mkv

4 directories, 2 files
```

### Copy or Move Instead of Linking

```bash
# Copy files (keeps originals)
mediar copy --tv-id tv_id --target /path/to/target /path/to/source

# Move files
mediar move --movie-id movie_id --target /path/to/target /path/to/source
```

## Supported File Formats

- Video: `.mp4`, `.mkv`, `.avi`, `.mov`, `.flv`, `.wmv`, `.webm`
- Subtitles: `.srt`

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
