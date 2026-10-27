# Contributing an Extension

Use [Load Unpacked…](get-started.md#2-load-it) to develop and share an extension
folder. This checkout implements marketplace browsing, installation, and updates,
but does not contain an extension publishing service or submission workflow.
The previously documented `murvix-app/extensions` repository is not currently
publicly accessible; do not rely on the old pull-request publishing instructions.

## Prepare a package

- Include `package.json` and every referenced entry, script, icon, and language
  bundle. Follow the current [manifest](manifest.md) and [schema](schema/manifest.schema.json).
- For a built extension, put the manifest and assets in `dist/`; if no `dist/`
  directory exists, Murvix uses the package folder directly. A build script is
  not required for a plain HTML/JavaScript extension.
- Use lowercase names (`a-z`, `0-9`, `.`, `-`, no leading `.`) for extensions
  with pages. An installed package's folder must match its `name`; an unpacked
  development folder need not.
- Use semver versions so marketplace update comparison can order them.
- Explain what the extension does and why it needs each permission. Keep the
  source readable and test the runtime surfaces you use on each target platform.

## Marketplace behavior

**Settings → Extensions → Browse** requests listings from the configured-in-code
`https://murvix.app` service. Availability and acceptance of new submissions
are controlled by that external service, not this repository. There is no
in-app package upload or publish command.

The installer requires a ZIP download from `https://murvix.app` on port 443.
The downloaded archive must match the listing's size and SHA-256 checksum. After
extraction, its manifest's name, version, and permissions must match the listing.
Archives cannot contain symlinks or escaping paths, and are limited to 256 MiB
compressed, 512 MiB expanded, and 40,000 entries.

An archive can contain `package.json` at its root or one containing directory.
Updates are downloaded and validated before the app replaces the existing
package. Users install available updates from **Settings → Extensions**.
These checks verify consistency with marketplace metadata, not code safety.

## Listing metadata

The loader ignores `murvix.marketplace`. Existing packages may use metadata
such as:

```json
"marketplace": {
  "author": "Your Name",
  "github": "your-handle",
  "categories": ["productivity"],
  "icon": "assets/icon.svg",
  "screenshots": ["assets/screenshot-1.png"]
}
```

This is not a verified specification for new submissions. Obtain current
publishing and listing-asset requirements from the marketplace operator before
preparing a submission; the local loader does not enforce screenshot dimensions,
pricing, or the presence of this block.
