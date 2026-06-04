import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background } from "../components/Background";
import { FeatureCard } from "../components/FeatureCard";

const DURATION = 150;
const FEATURE_ENTER = 18;

const PTTActivePulse: React.FC = () => {
  const frame = useCurrentFrame();
  const t = frame / 30;
  const pulse = 0.6 + 0.4 * Math.abs(Math.sin(t * Math.PI));
  return (
    <div
      style={{
        position: "absolute",
        inset: 0,
        borderRadius: 14,
        background: "rgba(255, 121, 198, 0.3)",
        opacity: pulse,
        pointerEvents: "none",
      }}
    />
  );
};

export const ComposerScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const exit = spring({
    frame: frame - (DURATION - 18),
    fps,
    config: { damping: 24, stiffness: 100, mass: 0.7 },
  });
  const exitOpacity = Math.max(0, 1 - Math.max(0, exit - 1));

  // Composer enter
  const composerEnter = spring({
    frame: frame - 8,
    fps,
    config: { damping: 18, stiffness: 90, mass: 0.7 },
  });
  const composerOpacity = interpolate(frame, [8, 28], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  // Typing in the textarea
  const prompt = "add a /health endpoint to the agent service that returns the orb mode and last turn duration";
  const typingSpeed = 1.2;
  const typedLen = Math.min(prompt.length, Math.max(0, Math.floor((frame - 35) * typingSpeed)));
  const typed = prompt.slice(0, typedLen);

  // PTT active window
  const pttActive = frame > 60 && frame < 130;

  // Send button morph
  const sendGlow = interpolate(frame, [100, 110, 120, 130], [0, 1, 1, 0.5], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <AbsoluteFill className="composer-scene" style={{ opacity: exitOpacity }}>
      <Background fadeIn={0} />
      <AbsoluteFill style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between", padding: "0 100px" }}>
        <FeatureCard
          feature={{
            id: "composer",
            kicker: "Composer + Voice",
            title: "Modern composer with Push-to-Talk",
            body: "Auto-growing textarea, footer model picker, plan/build modes, thinking level, and hold-to-speak transcription into any target.",
            accent: "pink",
          }}
          enterAt={FEATURE_ENTER}
          durationInFrames={DURATION}
          side="left"
        />

        <div
          style={{
            width: 640,
            opacity: composerOpacity,
            transform: `translateY(${(1 - composerEnter) * 30}px) scale(${0.95 + 0.05 * composerEnter})`,
          }}
        >
          {/* Header pill */}
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 10,
              padding: "8px 14px",
              borderRadius: 999,
              background: "rgba(31, 32, 48, 0.6)",
              border: "1px solid var(--border)",
              width: "fit-content",
              marginBottom: 12,
              fontFamily: "var(--font-mono)",
              fontSize: 16,
              color: "var(--text-muted)",
            }}
          >
            <span style={{ width: 8, height: 8, borderRadius: "50%", background: "var(--status-success)" }} />
            claude · opus 4.6
          </div>

          {/* Composer body */}
          <div
            style={{
              background: "var(--bg-panel)",
              border: "1px solid var(--border-strong)",
              borderRadius: 20,
              padding: 20,
              boxShadow: "0 30px 80px -20px rgba(0, 0, 0, 0.5)",
            }}
          >
            {/* Textarea */}
            <div
              style={{
                minHeight: 110,
                fontSize: 22,
                lineHeight: 1.5,
                color: "var(--text-bright)",
                fontFamily: "var(--font-sans)",
                padding: "8px 4px 16px 4px",
              }}
            >
              {typed}
              <span
                style={{
                  display: "inline-block",
                  width: 2,
                  height: 24,
                  background: "var(--accent-pink)",
                  marginLeft: 2,
                  verticalAlign: "text-bottom",
                  opacity: Math.sin(frame * 0.6) * 0.4 + 0.6,
                }}
              />
            </div>

            {/* Footer */}
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 10,
                paddingTop: 14,
                borderTop: "1px solid var(--border)",
              }}
            >
              <FooterChip label="Plan" />
              <FooterChip label="Standard" />
              <FooterChip label="Thinking ▾" />

              <div style={{ flex: 1 }} />

              <div style={{ position: "relative" }}>
                <button
                  style={{
                    width: 44,
                    height: 44,
                    borderRadius: 14,
                    background: pttActive ? "var(--accent-pink)" : "var(--bg-raised)",
                    border: "1px solid var(--border-strong)",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    color: pttActive ? "#16161e" : "var(--text-muted)",
                    fontSize: 20,
                  }}
                >
                  ●
                </button>
                {pttActive && <PTTActivePulse />}
              </div>

              <button
                style={{
                  width: 44,
                  height: 44,
                  borderRadius: 14,
                  background: `linear-gradient(135deg, var(--accent-pink) 0%, var(--accent-purple) 100%)`,
                  border: "none",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  color: "#16161e",
                  fontSize: 20,
                  fontWeight: 700,
                  boxShadow: `0 0 ${20 * sendGlow}px rgba(255, 121, 198, ${0.5 * sendGlow})`,
                }}
              >
                ↑
              </button>
            </div>
          </div>

          {/* Hint */}
          <div
            style={{
              marginTop: 12,
              fontFamily: "var(--font-mono)",
              fontSize: 14,
              color: "var(--text-faint)",
              letterSpacing: "0.04em",
            }}
          >
            ctrl+shift+space — push to talk
          </div>
        </div>
      </AbsoluteFill>
      <style>{`
        .composer-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

const FooterChip: React.FC<{ label: string }> = ({ label }) => (
  <div
    style={{
      padding: "8px 14px",
      borderRadius: 10,
      background: "var(--bg-raised)",
      border: "1px solid var(--border)",
      fontSize: 16,
      fontFamily: "var(--font-sans)",
      color: "var(--text)",
    }}
  >
    {label}
  </div>
);

export const COMPOSER_DURATION = DURATION;
