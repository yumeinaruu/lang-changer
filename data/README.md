# Word lists

`en_words.txt` and `ru_words.txt`: the 20,000 most frequent alphabetic words, most frequent first,
taken from the OpenSubtitles-2018 frequency lists in
https://github.com/hermitdave/FrequencyWords (content licensed CC-BY-SA 4.0).
Filtered to `[a-z]+` / `[а-яё]+`. They are embedded into the binary by `src/detector.rs`.
