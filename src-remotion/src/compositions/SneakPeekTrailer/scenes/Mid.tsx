import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background } from "../../ReleaseTrailer/components/Background";
import { FeatureCard } from "../../ReleaseTrailer/components/FeatureCard";

const DURATION = 150;
const FEATURE_ENTER = 18;

export const MidScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const exit = spring({
    frame: frame - (DURATION - 18),
    fps,
    config: { damping: 24, stiffness: 100, mass: 0.7 },
  });
  const exitOpacity = Math.max(0, 1 - Math.max(0, exit - 1));

  const orbEnter = spring({
    frame: frame - 8,
    fps,
    config: { damping: 18, stiffness: 90, mass: 0.7 },
  });
  const orbOpacity = interpolate(frame, [8, 28], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const pulse = 0.55 + 0.45 * Math.abs(Math.sin((frame / 30) * Math.PI));
  const ringSize = 320 + 40 * Math.sin((frame / 30) * Math.PI);

  return (
    <AbsoluteFill className="mid-scene" style={{ opacity: exitOpacity }}>
      <Background fadeIn={0} />
      <AbsoluteFill
        style={{
          flexDirection: "row",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "0 100px",
        }}
      >
        <FeatureCard
          feature={{
            id: "mid",
            kicker: "Mid scene",
            title: "A single feature, front and center",
            body: "Place-holder mid sequence. Reuse this scene as a template when extending the trailer with the next set of features.",
            accent: "cyan",
          }}
          enterAt={FEATURE_ENTER}
          durationInFrames={DURATION}
          side="left"
        />

        <div
          style={{
            width: 640,
            height: 640,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            opacity: orbOpacity,
            transform: `translateY(${(1 - orbEnter) * 30}px) scale(${0.95 + 0.05 * orbEnter})`,
          }}
        >
          <div
            style={{
              position: "relative",
              width: ringSize,
              height: ringSize,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <div
              style={{
                position: "absolute",
                inset: 0,
                borderRadius: "50%",
                border: "1px solid var(--accent-cyan)",
                opacity: 0.4,
                transform: `scale(${1 + 0.1 * pulse})`,
              }}
            />
            <div
              style={{
                position: "absolute",
                inset: 36,
                borderRadius: "50%",
                border: "1px solid var(--accent-purple)",
                opacity: 0.3,
                transform: `scale(${1 - 0.08 * pulse})`,
              }}
            />
            <div
              style={{
                width: 200,
                height: 200,
                borderRadius: "50%",
                background:
                  "radial-gradient(circle at 30% 30%, var(--accent-cyan) 0%, var(--accent-purple) 55%, var(--accent-pink) 100%)",
                boxShadow:
                  "0 30px 80px -20px rgba(125, 207, 255, 0.5), 0 0 0 1px rgba(189, 200, 245, 0.06) inset",
                filter: `brightness(${0.85 + 0.25 * pulse})`,
              }}
            />
          </div>
        </div>
      </AbsoluteFill>
      <style>{`
        .mid-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const MID_DURATION = DURATION;
