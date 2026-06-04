# src-remotion

Standalone [Remotion](https://www.remotion.dev/) 4 project for blxcode video output.
The directory is excluded from the parent repo's git tracking (see root `.gitignore`).

## Quick start

```bash
cd src-remotion
npm install
npm start                       # open the Remotion Studio at http://localhost:3000
npm run build                   # render HelloWorld → out/hello.mp4
npm run build:image             # render a still frame → out/hello.png
npm run build:trailer           # render ReleaseTrailer → out/release-trailer.mp4
npm run build:trailer:image     # render a still frame at frame 200
npm run build:sneak             # render SneakPeekTrailer → out/sneak-peek-trailer.mp4
npm run build:sneak:image       # render a still frame at frame 200
npm run typecheck
```

## Compositions

| id | Size | Duration | Notes |
|---|---|---|---|
| `HelloWorld` | 1280×720 | 5 s | Minimal smoke-test composition |
| `ReleaseTrailer` | 1920×1080 | 50 s | 10-scene announcement trailer for the next blxcode release, sourced from the repo's `CHANGELOG.md` `[Unreleased]` section. |
| `SneakPeekTrailer` | 1920×1080 | 15 s | Minimal 3-scene teaser (Start / Mid / End) based on `ReleaseTrailer`. Reuses `Background` + `FeatureCard`; meant to grow as more scenes land. |

## Layout

```
src-remotion/
├── package.json
├── tsconfig.json
├── remotion.config.ts
├── public/                # static assets (fonts, images, audio)
└── src/
    ├── index.ts           # entry point: registerRoot(Root)
    ├── Root.tsx           # composition registry
    └── compositions/
        ├── HelloWorld/
        │   ├── HelloWorld.tsx
        │   └── styles.css
        ├── ReleaseTrailer/
        │   ├── ReleaseTrailer.tsx       # orchestrator (Sequence-per-scene)
        │   ├── ReleaseTrailer.css       # shared trailer styles
        │   ├── data.ts                  # feature list + accent palette
        │   ├── tokens.ts                # default app color tokens
        │   ├── components/
        │   │   ├── Background.tsx       # animated mesh bg + orbs
        │   │   └── FeatureCard.tsx      # reusable kicker / title / body card
        │   └── scenes/
        │       ├── Intro.tsx
        │       ├── Theme.tsx            # Tokyo Night × Dracula reveal
        │       ├── Orb.tsx              # 3D Drobo + thinking stream
        │       ├── Composer.tsx         # modern composer + PTT
        │       ├── GitGraph.tsx         # VS Code-style commit graph
        │       ├── Terminals.tsx        # named terminals
        │       ├── Plans.tsx            # AI plan dialog
        │       ├── Roundings.tsx        # radius + font picker
        │       ├── Themes.tsx           # 32-theme grid
        │       └── Outro.tsx
        └── SneakPeekTrailer/
            ├── SneakPeekTrailer.tsx     # 3-scene orchestrator (Start / Mid / End)
            └── scenes/
                ├── Start.tsx            # title + version pill
                ├── Mid.tsx              # feature card + pulse orb
                └── End.tsx              # logo + CTA + credits
```

The trailer uses only the default `blxcode-dark` theme tokens
(`themes/tokens.css` in the parent repo) — no extra CSS imports, no image
assets, so it can render fully offline.
