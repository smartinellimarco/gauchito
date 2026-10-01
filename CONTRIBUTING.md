# Contributing

Thanks for helping out. Bugs and ideas go to the issue tracker; code goes
through pull requests.

## Workflow

1. Fork the repo and create a branch named `type/short-description`, for
   example `feat/crdt-buffer` or `fix/end-key-empty-line`.
2. Commit however you like; your commits are squashed on merge.
3. Open a pull request whose title follows
   [Conventional Commits](https://www.conventionalcommits.org):

   ```
   type(scope): short summary in the imperative
   ```

   - `type` is one of `feat`, `fix`, `perf`, `refactor`, `docs`, `test`,
     `build`, `ci`, `chore`, `style`, `revert`.
   - `scope` is optional and names the crate touched: `core`, `ui`,
     `script`, `cli`, or `deps` for dependency bumps.
   - Breaking changes add `!` after the type or scope (`feat(script)!: ...`)
     and explain the migration in the description.

   The title becomes the commit message on `main` and the changelog entry,
   so write it for users. The description becomes the commit body.
4. CI must pass and a maintainer must approve before merging.

## Checks

Run what CI runs before pushing:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Releases

Maintainers release by merging the release PR that release-plz keeps up to
date. Versions follow [Semantic Versioning](https://semver.org); the
changelog is generated from the pull request titles.

## License

By contributing you agree that your contributions are licensed under the
MIT license of this repository.
