import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background } from "../components/Background";

const DURATION = 150;

export const IntroScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const titleEnter = spring({
    frame,
    fps,
    config: { damping: 14, stiffness: 80, mass: 0.8 },
  });

  const kickerOpacity = interpolate(frame, [10, 28], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const titleOpacity = interpolate(frame, [18, 40], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const subtitleOpacity = interpolate(frame, [50, 75], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const versionOpacity = interpolate(frame, [75, 95], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const versionY = interpolate(frame, [75, 95], [20, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const exit = spring({
    frame: frame - (DURATION - 18),
    fps,
    config: { damping: 24, stiffness: 100, mass: 0.7 },
  });
  const exitOpacity = Math.max(0, 1 - Math.max(0, exit - 1));

  return (
    <AbsoluteFill className="intro-scene" style={{ opacity: exitOpacity }}>
      <Background fadeIn={0} />
      <AbsoluteFill style={{ alignItems: "center", justifyContent: "center" }}>
        <div className="intro__stack">
          <div
            className="intro__kicker"
            style={{
              opacity: kickerOpacity,
              transform: `translateY(${(1 - kickerOpacity) * 12}px)`,
            }}
          >
            <span className="intro__kicker__dot" />
            blxcode — release trailer
          </div>
          <h1
            className="intro__title"
            style={{
              opacity: titleOpacity,
              transform: `scale(${0.92 + 0.08 * titleEnter}) translateY(${(1 - titleEnter) * 24}px)`,
            }}
          >
            <span className="intro__title__word">what&rsquo;s</span>
            <span className="intro__title__word intro__title__word--accent">new</span>
          </h1>
          <p
            className="intro__subtitle"
            style={{
              opacity: subtitleOpacity,
              transform: `translateY(${(1 - subtitleOpacity) * 16}px)`,
            }}
          >
            custom titlebar · 3D drobo orb · push-to-talk · per-terminal CLI agents · AI plans · session stats · tool-loop limit · CodeMirror preview · memory center · 32 themes
          </p>
          <div
            className="intro__version"
            style={{
              opacity: versionOpacity,
              transform: `translateY(${versionY}px)`,
            }}
          >
            <span className="intro__version__label">unreleased</span>
            <span className="intro__version__sep">/</span>
            <span className="intro__version__date">v0.3.4+</span>
          </div>
        </div>
      </AbsoluteFill>
      <style>{`
        .intro-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
        .intro__stack {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 28px;
          text-align: center;
          padding: 0 80px;
        }
        .intro__kicker {
          display: inline-flex;
          align-items: center;
          gap: 12px;
          font-family: var(--font-mono);
          font-size: 22px;
          font-weight: 500;
          letter-spacing: 0.14em;
          text-transform: uppercase;
          color: var(--text-muted);
          white-space: nowrap;
        }
        .intro__kicker__dot {
          width: 10px;
          height: 10px;
          border-radius: 50%;
          background: var(--accent-purple);
          box-shadow: 0 0 14px var(--accent-purple);
        }
        .intro__title {
          font-size: 220px;
          font-weight: 800;
          line-height: 0.95;
          letter-spacing: -0.04em;
          margin: 0;
          color: var(--text-bright);
          display: flex;
          gap: 32px;
          align-items: baseline;
        }
        .intro__title__word--accent {
          background: linear-gradient(135deg, var(--accent-purple) 0%, var(--accent-cyan) 60%, var(--accent-pink) 100%);
          -webkit-background-clip: text;
          background-clip: text;
          -webkit-text-fill-color: transparent;
          color: transparent;
        }
        .intro__subtitle {
          font-size: 26px;
          font-weight: 400;
          line-height: 1.5;
          color: var(--text-muted);
          margin: 0;
          max-width: 1700px;
        }
        .intro__version {
          display: inline-flex;
          align-items: center;
          gap: 14px;
          padding: 12px 22px;
          border-radius: 999px;
          border: 1px solid var(--border-strong);
          background: rgba(22, 22, 30, 0.6);
          font-family: var(--font-mono);
          font-size: 20px;
          color: var(--text);
        }
        .intro__version__label {
          color: var(--accent-purple);
          font-weight: 600;
        }
        .intro__version__sep {
          color: var(--text-faint);
        }
        .intro__version__date {
          color: var(--text-muted);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const INTRO_DURATION = DURATION;
