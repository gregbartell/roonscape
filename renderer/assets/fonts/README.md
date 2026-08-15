# Packaged fallback typography

RoonScape packages only its open fallback pair. The preferred Palatino
Linotype and Segoe UI faces are host-provided and must not be copied into this
directory or redistributed with RoonScape.

The four variable TrueType files were copied without modification, with their
two OFL notices, from the Google Fonts repository at commit
`352f6b7d9d6cc4fa9e242b931291d31b21a6dc84`:

- `ofl/librebaskerville/LibreBaskerville[wght].ttf`
- `ofl/librebaskerville/LibreBaskerville-Italic[wght].ttf`
- `ofl/librebaskerville/OFL.txt`
- `ofl/ibmplexsans/IBMPlexSans[wdth,wght].ttf`
- `ofl/ibmplexsans/IBMPlexSans-Italic[wdth,wght].ttf`
- `ofl/ibmplexsans/OFL.txt`

At startup, the renderer registers these files as application fonts through
Fontconfig. They remain private to the renderer process and require neither a
network request nor a global host installation.
