# Releasing Ramure

A version of Ramure is a `v*` tag. The workflow `.github/workflows/release.yml` builds its
installers on Windows and on Ubuntu and puts them in a draft GitHub release. The version is public
once the draft is published by hand.

## Cut a release

1. On `main`, up to date, set the version. It follows semantic versioning: three numbers, as in
   `0.2.0`.

   ```bash
   npm run version:set -- 0.2.0
   ```

   The script writes it in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`,
   `src-tauri/Cargo.lock` and `src-tauri/tauri.conf.json`. `npm run check` fails when they
   disagree.

2. Commit that change through a pull request, as any other, and merge it.

3. Tag the merge commit and push the tag.

   ```bash
   git switch main && git pull
   git tag v0.2.0
   git push origin v0.2.0
   ```

4. The "Release" workflow starts. It checks that the tag is the version of the application, runs
   `npm run check` and `cargo test` on both systems, builds the installers, and uploads them to a
   draft release named "Ramure 0.2.0", with notes generated from the pull requests merged since
   the last version. It takes about half an hour.

5. Open the draft on the [releases page](https://github.com/kihak12/Ramure/releases). Install one
   of its files and start the application. Edit the notes, then choose "Publish release".

## What a release holds

| File | For |
| --- | --- |
| `Ramure_0.2.0_x64-setup.exe` | Windows 10 and 11. Installs for the current user, without administrator rights. Downloads WebView2 if it is missing. |
| `Ramure_0.2.0_amd64.deb` | Ubuntu 22.04 and 24.04. Depends on `git`. |
| `Ramure_0.2.0_amd64.AppImage` | Other Linux distributions, or a trial without installing. |

The Linux files are built on Ubuntu 22.04, the oldest supported: what is built there runs on
24.04 too.

## When a release fails

- **The tag is not the version.** The first job says which files disagree. Delete the tag
  (`git push origin :v0.2.0`), fix the version through a pull request, and tag again.
- **A build fails.** Fix it through a pull request, delete the tag and the draft, and tag the
  new commit. To run a job again without a new commit, use "Re-run failed jobs" on the run: the
  draft is kept and the files are replaced.
- **A version that was published is wrong.** Do not replace its files: publish the next patch
  version.

## Try the build without releasing

A pull request that touches the workflow, the version script, `package.json`, the lock files,
`Cargo.toml` or `tauri.conf.json` runs the same build and publishes nothing. The installers are
kept for a time as artifacts of the run, at the bottom of its page. "Run workflow", on the
"Release" workflow of the "Actions" tab, does the same on any branch.

## Not there yet

- The Windows installer is not signed. On the first run, Windows shows "Windows protected your
  PC": "More info", then "Run anyway". A code-signing certificate is what removes it.
