# Releasing

Use this checklist after the release version has been approved.

1. Set the same version in `package.json`, `src/bridge/package.json`,
   `src/renderer/Cargo.toml`, the extension identity in
   `src/bridge/src/roon-extension.ts`, and the release-package expectation in
   `scripts/package-release.test.mjs`. Refresh `package-lock.json` and
   `Cargo.lock` so their RoonScape entries agree with that version. The complete
   repository check rejects any inconsistency across these sources.
2. Install the locked development dependencies with `npm ci`, then run the
   complete local check and the authoritative release-package command:

   ```sh
   npm run check
   npm run package
   ```

   The repository check uses controlled packaging inputs and does not build or
   download release-package components. `npm run package` is the explicit
   end-to-end release check: it builds the Roon bridge and renderer, installs
   production dependencies, downloads and verifies the pinned Node runtime,
   assembles the archive once, and fully validates that same archive and its
   checksum. It leaves the validated artifacts under `release/`.

3. Review the changes and the two files under `release/`, then commit the
   release. The expected files are `roonscape-linux-x64.tar.gz` and
   `roonscape-linux-x64.tar.gz.sha256`.
4. Create a tag whose name is exactly `v` followed by the package version. For
   version `1.0.0`, use:

   ```sh
   git tag v1.0.0
   ```

5. Obtain explicit authorization from the release owner before pushing either
   the release commit or tag. Once authorized, push the commit and then the
   matching tag.
6. Wait for the tag-triggered workflow to finish. Confirm that the GitHub
   Release is public, has generated release notes, and contains only the stable
   Linux archive and checksum named above.
7. Download both assets through their stable latest-release URLs, verify the
   checksum, extract the archive, and exercise the packaged launcher:

   ```sh
   curl --fail --location --output roonscape-linux-x64.tar.gz \
     https://github.com/gregbartell/roonscape/releases/latest/download/roonscape-linux-x64.tar.gz
   curl --fail --location --output roonscape-linux-x64.tar.gz.sha256 \
     https://github.com/gregbartell/roonscape/releases/latest/download/roonscape-linux-x64.tar.gz.sha256
   sha256sum --check roonscape-linux-x64.tar.gz.sha256
   tar --extract --gzip --file roonscape-linux-x64.tar.gz
   ./roonscape/roonscape --help
   ```
