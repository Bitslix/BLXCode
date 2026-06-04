import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background, DroboOrb3D, FeatureCard } from "../components";

const DURATION = 150;
const FEATURE_ENTER = 18;

export const OrbScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const exit = spring({
    frame: frame - (DURATION - 18),
    fps,
    config: { damping: 24, stiffness: 100, mass: 0.7 },
  });
  const exitOpacity = Math.max(0, 1 - Math.max(0, exit - 1));

  const orbOpacity = interpolate(frame, [0, 18, DURATION - 18, DURATION], [0, 1, 1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const orbScale = spring({
    frame,
    fps,
    config: { damping: 16, stiffness: 80, mass: 0.9 },
  });

  const thinkingLines = [
    "let me check the architecture map for the rust workspace",
    "looking at agent_panel/mod.rs to see the composer wiring",
    "yes, the modern composer lives in agent_panel/composer",
  ];
  const typedChars = thinkingLines.map((line, i) => {
    const start = 35 + i * 18;
    const elapsed = Math.max(0, frame - start);
    return Math.min(line.length, Math.floor(elapsed * 0.8));
  });
  const thinkingOpacity = interpolate(frame, [30, 45], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <AbsoluteFill className="orb-scene" style={{ opacity: exitOpacity }}>
      <Background fadeIn={0} />
      <AbsoluteFill
        style={{
          flexDirection: "row",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "0 100px",
        }}
      >
        <div
          style={{
            position: "relative",
            width: 540,
            height: 540,
            opacity: orbOpacity,
            transform: `scale(${0.85 + 0.15 * orbScale})`,
          }}
        >
          {/* Soft halo behind orb */}
          <div
            style={{
              position: "absolute",
              inset: 60,
              borderRadius: "50%",
              background:
                "radial-gradient(circle, rgba(189, 147, 249, 0.32) 0%, rgba(125, 207, 255, 0.12) 45%, transparent 70%)",
              filter: "blur(40px)",
            }}
          />
          <DroboOrb3D accent="#bd93f9" accent2="#7dcfff" size={540} />
          {/* Tracking cursor reticle */}
          <div
            style={{
              position: "absolute",
              inset: 0,
              border: "1px dashed rgba(189, 147, 249, 0.15)",
              borderRadius: "50%",
              pointerEvents: "none",
            }}
          />
        </div>

        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "flex-end",
            gap: 32,
            maxWidth: 600,
          }}
        >
          <FeatureCard
            feature={{
              id: "orb",
              kicker: "Agent",
              title: "3D Drobo agent orb",
              body: "Real Three.js model from Drobo.glb, recolored from theme tokens, follows the cursor, reacts to states. 2D fallback still available.",
              accent: "cyan",
            }}
            enterAt={FEATURE_ENTER}
            durationInFrames={DURATION}
            side="right"
          />
          <div
            style={{
              width: 560,
              padding: 24,
              borderRadius: 16,
              background: "rgba(22, 22, 30, 0.7)",
              border: "1px solid var(--border-strong)",
              fontFamily: "var(--font-mono)",
              fontSize: 18,
              color: "var(--text-muted)",
              lineHeight: 1.7,
              opacity: thinkingOpacity,
              transform: `translateY(${(1 - thinkingOpacity) * 20}px)`,
            }}
          >
            <div
              style={{
                fontSize: 14,
                fontWeight: 500,
                color: "var(--accent-cyan)",
                letterSpacing: "0.14em",
                textTransform: "uppercase",
                marginBottom: 12,
              }}
            >
              ▾ thinking stream
            </div>
            {thinkingLines.map((line, i) => (
              <div key={i} style={{ display: "flex", gap: 10 }}>
                <span style={{ color: "var(--text-faint)" }}>{i + 1}.</span>
                <span style={{ color: "var(--text)" }}>
                  {line.slice(0, typedChars[i])}
                  {i === thinkingLines.length - 1 && typedChars[i] === line.length ? null : (
                    <span
                      style={{
                        display: "inline-block",
                        width: 8,
                        height: 18,
                        background: "var(--accent-cyan)",
                        marginLeft: 2,
                        verticalAlign: "text-bottom",
                        opacity: Math.sin(frame / 8) * 0.5 + 0.5,
                      }}
                    />
                  )}
                </span>
              </div>
            ))}
          </div>
        </div>
      </AbsoluteFill>
      <style>{`
        .orb-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const ORB_DURATION = DURATION;
