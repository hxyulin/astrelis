# Text fixtures

These unmodified fonts are embedded only in tests, the headless text example, and
the CPU benchmark. The library does not embed fonts or implicitly discover system
fonts. Both fonts use the SIL Open Font License 1.1; their accompanying license
files retain the upstream copyright and reserved-name declarations.

- [Source Sans 3 regular OTF](https://github.com/adobe-fonts/source-sans/blob/7c691c2772570a0d3a9e1bffe8a9d6074257d985/OTF/SourceSans3-Regular.otf)
  exercises Latin kerning, ligatures, and combining marks. Its license is
  [SourceSans3-OFL.md](SourceSans3-OFL.md), from the same repository's
  [license revision](https://github.com/adobe-fonts/source-sans/blob/ca29c267aed34e82052d7ca5e027cf0e714c0f11/LICENSE.md).
- [Noto Sans Arabic variable TTF](https://github.com/google/fonts/blob/e5ee31305efa101c2cae65f44d9642df2c8f0b4d/ofl/notosansarabic/NotoSansArabic%5Bwdth,wght%5D.ttf)
  exercises Arabic joining, mixed bidi, and fallback. Its license is
  [NotoSansArabic-OFL.txt](NotoSansArabic-OFL.txt), from that same revision.

[sources.json](sources.json) records pinned upstream revisions, original paths,
byte lengths, and SHA-256 hashes. Tests do not download fonts or depend on installed
fonts. The collection test packages the real TTF in memory as a two-face TTC sharing
tables, adjusting the container's absolute offsets; no additional binary fixture
or custom font parser is needed.
