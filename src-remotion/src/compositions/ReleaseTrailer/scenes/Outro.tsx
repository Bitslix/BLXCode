import { AbsoluteFill, Img, interpolate, spring, staticFile, useCurrentFrame, useVideoConfig } from "remotion";
import { Background } from "../components/Background";

const DURATION = 150;

export const OutroScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const logoEnter = spring({
    frame,
    fps,
    config: { damping: 14, stiffness: 70, mass: 0.9 },
  });

  const ctaOpacity = interpolate(frame, [25, 45], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const ctaY = interpolate(frame, [25, 45], [16, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  const tagOpacity = interpolate(frame, [55, 75], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const tagY = interpolate(frame, [55, 75], [16, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  const creditOpacity = interpolate(frame, [85, 105], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  return (
    <AbsoluteFill className="outro-scene">
      <Background fadeIn={0} />

      {/* Centerline glow */}
      <div
        style={{
          position: "absolute",
          left: "50%",
          top: "50%",
          transform: "translate(-50%, -50%)",
          width: 600,
          height: 600,
          background: "radial-gradient(circle, rgba(189, 147, 249, 0.18) 0%, transparent 65%)",
          filter: "blur(40px)",
        }}
      />

      <AbsoluteFill style={{ alignItems: "center", justifyContent: "center" }}>
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            gap: 32,
            textAlign: "center",
          }}
        >
          {/* Logo */}
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 24,
              opacity: logoEnter,
              transform: `scale(${0.85 + 0.15 * logoEnter})`,
            }}
          >
            <Img
              src={staticFile("blxcode.png")}
              style={{
                width: 100,
                height: 100,
                borderRadius: 24,
                background: "linear-gradient(135deg, var(--accent-purple) 0%, var(--accent-cyan) 50%, var(--accent-pink) 100%)",
                objectFit: "cover",
                boxShadow: "0 20px 50px -10px rgba(189, 147, 249, 0.5)",
              }}
            />
            <div
              style={{
                fontSize: 110,
                fontWeight: 800,
                lineHeight: 1,
                letterSpacing: "-0.04em",
                color: "var(--text-bright)",
                fontFamily: "var(--font-sans)",
              }}
            >
              blxcode
            </div>
          </div>

          {/* CTA */}
          <div
            style={{
              opacity: ctaOpacity,
              transform: `translateY(${ctaY}px)`,
            }}
          >
            <div
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: 12,
                padding: "16px 32px",
                borderRadius: 999,
                background: "linear-gradient(135deg, var(--accent-purple) 0%, var(--accent-pink) 100%)",
                color: "#16161e",
                fontFamily: "var(--font-sans)",
                fontSize: 32,
                fontWeight: 700,
                boxShadow: "0 16px 40px -8px rgba(189, 147, 249, 0.5)",
              }}
            >
              <span
                style={{
                  width: 12,
                  height: 12,
                  borderRadius: "50%",
                  background: "#16161e",
                  boxShadow: "0 0 8px rgba(0, 0, 0, 0.5)",
                }}
              />
              available now
            </div>
          </div>

          {/* Tag */}
          <div
            style={{
              opacity: tagOpacity,
              transform: `translateY(${tagY}px)`,
              fontFamily: "var(--font-mono)",
              fontSize: 22,
              color: "var(--text-muted)",
              letterSpacing: "0.06em",
            }}
          >
            github.com/anomalyco/blxcode
          </div>

          {/* Credits */}
          <div
            style={{
              position: "absolute",
              bottom: 60,
              left: 0,
              right: 0,
              opacity: creditOpacity,
              fontFamily: "var(--font-mono)",
              fontSize: 16,
              color: "var(--text-faint)",
              letterSpacing: "0.16em",
              textTransform: "uppercase",
            }}
          >
            blxcode v0.3.4 — release trailer · 2026
          </div>
        </div>
      </AbsoluteFill>

      <style>{`
        .outro-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const OUTRO_DURATION = DURATION;
