# Adding a golden test

[← docs index](../README.md)

1. Create `tests/golden/NAME.in.jsonl` with one command per line.
2. Create `tests/golden/NAME.out.jsonl` with the expected response on the **same line number**.
   Blank lines in either file are ignored.
3. Add `NAME` to the `golden!` list in `tests/golden.rs`:

   ```rust
   golden! {
       base_example,
       NAME,
       #[ignore = "reason"]   // optional, for cases that can't pass yet
       other_case,
   }
   ```

4. Run `cargo test --test golden`.

If you skip step 3, `all_fixtures_registered` fails and names the missing fixture.

## Tips

- Responses are compared as parsed JSON. Object key order doesn't matter, but array order
  (cargo lists) does.
- A failure message shows the line number, the command, and the got/want values. Fix one line at
  a time from the top, since later lines depend on earlier state.
- Each fixture runs in a fresh `Engine`, so a fixture should start with `init` unless it's
  testing pre-init errors.
