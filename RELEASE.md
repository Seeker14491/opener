# Release Procedure

1. Update version number in Cargo.toml
2. Update version number in html_root_url attribute
3. Update CHANGELOG.md
4. Commit changes
5. `cd` into the `opener` subdirectory that contains `Drakefile.ts` and, using [Deno](https://deno.land/), run `deno run -A Drakefile.ts release version=a.b.c`, substituting the correct version