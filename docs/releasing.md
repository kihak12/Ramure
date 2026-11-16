# Releasing Ramure

A version of Ramure is a `v*` tag. The workflow `.github/workflows/release.yml` builds its
installers on Windows and on Ubuntu and puts them in a draft GitHub release. The version is public
once the draft is published by hand.

## Once: the key of the updates

An installed copy only accepts an update signed with the key of the project (guidelines, 9.23).
The key is made once, by the maintainer, on their own machine.

1. Make the key pair, with a password:

   ```bash
   npm run tauri signer generate -- -w ~/.tauri/ramure.key
   ```

   It writes the private key to `~/.tauri/ramure.key` and the public key to
   `~/.tauri/ramure.key.pub`.

2. Put the content of `ramure.key.pub`, one line, in `plugins.updater.pubkey` of
   `src-tauri/tauri.conf.json`, and commit it. The public key is not a secret.

3. In the repository on GitHub, under "Settings", "Secrets and variables", "Actions", add two
   repository secrets:
   - `TAURI_SIGNING_PRIVATE_KEY`: the content of `~/.tauri/ramure.key`;
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: its password.

4. Keep the private key and its password somewhere safe, outside the repository. **Whoever has
   them can publish an update that every installed copy accepts. If they are lost, no installed
   copy can update any more:** every user has to install a new version by hand, built with a new
   key.

A tag is refused while the public key is missing.

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

   Do not create the release on GitHub by hand, before or after the tag: the workflow makes it.

4. The "Release" workflow starts. It checks that the tag is the version of the application, runs
   `npm run check` and `cargo test` on both systems, builds the installers, signs them, and
   uploads them to a draft release named "Ramure 0.2.0", with notes generated from the pull
   requests merged since the last version.

5. Open the draft the workflow made, "Ramure 0.2.0", on the
   [releases page](https://github.com/kihak12/Ramure/releases): its pencil, not "Draft a new
   release". It holds seven files. Install one of them and start the application. Edit the
   notes, then choose "Publish release". From then on, the installed copies find the new version
   at their next check.

   Until it is published, a copy of that version that checks for updates says "No published
   version was found on GitHub.": the files of a draft are not public.

## What a release holds

| File | For |
| --- | --- |
| `Ramure_0.2.0_x64-setup.exe` | Windows 10 and 11. Installs for the current user, without administrator rights. Downloads WebView2 if it is missing. |
| `Ramure_0.2.0_amd64.deb` | Ubuntu 22.04 and 24.04. Depends on `git`. |
| `Ramure_0.2.0_amd64.AppImage` | Other Linux distributions, or a trial without installing. |
| `*.sig` | The signature of each installer, which an installed copy checks before it installs an update. |
| `latest.json` | What an installed copy reads to know the last version: its number, its date, and where its installers and their signatures are. |

An installed copy updates from the installer of its own kind: a copy installed from the `.deb`
downloads the `.deb`, an AppImage downloads the AppImage.

The Linux files are built on Ubuntu 22.04, the oldest supported: what is built there runs on
24.04 too.

## When a release fails

- **The tag is not the version.** The first job says which files disagree. Delete the tag
  (`git push origin :v0.2.0`), fix the version through a pull request, and tag again.
- **A build fails.** Fix it through a pull request, delete the tag and the draft, and tag the
  new commit. To run a job again without a new commit, use "Re-run failed jobs" on the run: the
  draft is kept and the files are replaced.
- **A version that was published is wrong.** Do not replace its files: publish the next patch
  version. Installed copies only move to a higher version.
- **The signing fails.** "A public key has been found, but no private key": one of the two
  secrets is missing or misspelled. "Missing comment in secret key": the secret
  `TAURI_SIGNING_PRIVATE_KEY` does not hold the whole content of the key file. Fix the secret,
  then "Re-run failed jobs".
- **Installed copies say "No published version was found on GitHub."** The last published
  release has no `latest.json`. Either the draft of the workflow is not published yet, or a
  second release was created by hand for the same tag and published in its place, empty. Delete
  the empty release, keeping its tag, then publish the draft of the workflow.

## Try the build without releasing

A pull request that touches the workflow, the version script, `package.json`, the lock files,
`Cargo.toml` or `tauri.conf.json` runs the same build and publishes nothing. It signs nothing
either, and needs no secret. The installers are kept for a time as artifacts of the run, at the
bottom of its page. "Run workflow", on the
"Release" workflow of the "Actions" tab, does the same on any branch.

## Not there yet

- The Windows installer is not signed. On the first run, Windows shows "Windows protected your
  PC": "More info", then "Run anyway". A code-signing certificate is what removes it.
