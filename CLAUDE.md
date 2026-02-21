# Bisque Computer

A native macOS dashboard client built with Rust (vello/winit).

## Post-Change Requirement

**IMPORTANT:** After ANY change is committed and pushed, you MUST send the user instructions to re-download and open the latest build. Include this in your message:

### For Apple Silicon (M1/M2/M3/M4):
```
curl -L https://github.com/aeschylus/bisque-computer/releases/latest/download/BisqueComputer-aarch64-apple-darwin.tar.gz -o /tmp/BisqueComputer.tar.gz && tar xzf /tmp/BisqueComputer.tar.gz -C /tmp && open /tmp/BisqueComputer.app
```

### For Intel Mac:
```
curl -L https://github.com/aeschylus/bisque-computer/releases/latest/download/BisqueComputer-x86_64-apple-darwin.tar.gz -o /tmp/BisqueComputer.tar.gz && tar xzf /tmp/BisqueComputer.tar.gz -C /tmp && open /tmp/BisqueComputer.app
```

**Note:** A new release must be tagged for these downloads to reflect the latest changes. After pushing to main, tag a new version and push the tag to trigger the release workflow:
```
git tag vX.Y.Z && git push origin vX.Y.Z
```

## Design
- All backgrounds: bisque beige
- All text: black (#000000)
- Readable text font: Optima (with serif fallbacks)
- Code/mono font: Monaco (with monospace fallbacks)
- Text sizes are doubled from original defaults
