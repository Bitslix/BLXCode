import { AbsoluteFill, spring, useCurrentFrame, useVideoConfig } from "remotion";
import type { Feature } from "../data";

export type FeatureCardProps = {
  feature: Feature;
  /** Frame at which the card should enter */
  enterAt: number;
  /** Total scene duration in frames */
  durationInFrames: number;
  /** Layout: "right" = card on right, visual on left; "left" = card on left */
  side?: "right" | "left";
};

const accentVar = {
  purple: "var(--accent-purple)",
  cyan: "var(--accent-cyan)",
  pink: "var(--accent-pink)",
  blue: "var(--accent-blue)",
  success: "var(--status-success)",
  warning: "var(--status-warning)",
  danger: "var(--status-danger)",
} as const;

export const FeatureCard: React.FC<FeatureCardProps> = ({
  feature,
  enterAt,
  durationInFrames,
  side = "right",
}) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const enter = spring({
    frame: frame - enterAt,
    fps,
    config: { damping: 18, stiffness: 90, mass: 0.7 },
  });

  const exit = spring({
    frame: frame - (durationInFrames - 18),
    fps,
    config: { damping: 24, stiffness: 100, mass: 0.7 },
  });

  const opacity = enter * (1 - Math.max(0, exit - 1));
  const translateX = (1 - enter) * (side === "right" ? 60 : -60);
  const accent = accentVar[feature.accent];

  return (
    <div
      className="feature-card"
      style={{
        opacity,
        transform: `translateX(${translateX}px)`,
      }}
    >
      <div className="feature-card__kicker" style={{ color: accent }}>
        {feature.kicker}
      </div>
      <h2 className="feature-card__title">{feature.title}</h2>
      <p className="feature-card__body">{feature.body}</p>
      <div className="feature-card__rule" style={{ background: accent }} />
      <style>{`
        .feature-card {
          width: 560px;
          padding: 36px 40px;
          background: linear-gradient(180deg, rgba(31, 32, 48, 0.92) 0%, rgba(26, 27, 38, 0.85) 100%);
          border: 1px solid var(--border-strong);
          border-radius: 20px;
          box-shadow:
            0 30px 80px -20px rgba(0, 0, 0, 0.6),
            0 0 0 1px rgba(189, 200, 245, 0.04) inset;
          backdrop-filter: blur(12px);
          position: relative;
          overflow: hidden;
        }
        .feature-card__kicker {
          font-family: var(--font-mono);
          font-size: 20px;
          font-weight: 500;
          letter-spacing: 0.14em;
          text-transform: uppercase;
          margin-bottom: 18px;
        }
        .feature-card__title {
          font-family: var(--font-sans);
          font-size: 54px;
          font-weight: 700;
          line-height: 1.05;
          letter-spacing: -0.022em;
          color: var(--text-bright);
          margin: 0 0 18px 0;
        }
        .feature-card__body {
          font-size: 24px;
          font-weight: 400;
          line-height: 1.5;
          color: var(--text-muted);
          margin: 0;
        }
        .feature-card__rule {
          position: absolute;
          left: 0;
          top: 0;
          width: 4px;
          height: 100%;
        }
      `}</style>
    </div>
  );
};

export const FeatureCardSlot: React.FC<{ side: "right" | "left"; children: React.ReactNode }> = ({
  side,
  children,
}) => (
  <AbsoluteFill style={{ justifyContent: "center", alignItems: side === "right" ? "flex-end" : "flex-start", padding: side === "right" ? "0 100px 0 0" : "0 0 0 100px" }}>
    {children}
  </AbsoluteFill>
);
