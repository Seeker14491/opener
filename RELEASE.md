# Release Procedure

1. Update version number in Cargo.toml
2. Update CHANGELOG.md
3. Commit changes
4. `cd` into the `opener` subdirectory that contains `Drakefile.ts` and, using [Deno](https://deno.land/), run `deno run -A Drakefile.ts release version=a.b.c`, substituting the correct version
