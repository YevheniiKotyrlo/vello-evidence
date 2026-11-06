# vello evidence

Images and programs behind the evidence comments on my [linebender/vello](https://github.com/linebender/vello) pull requests.

- `images/pr-<N>-<slug>-before.png` is rendered with vello at the pull request's base, and `-after.png` with its branch, by the program in `captures/pr-<N>-<slug>-pair/`; nothing else differs between the two.
- `captures/pr-<N>-<slug>-<program>/` holds each program behind a comment's numbers, a Cargo project of its `Cargo.toml` and `main.rs`.

Each pull request's comment says what its images show and how its numbers were made.
