import { AbsoluteFill, spring, useCurrentFrame, useVideoConfig } from "remotion";
import "./styles.css";

export const HelloWorld: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const scale = spring({
    frame,
    fps,
    config: {
      damping: 12,
      stiffness: 80,
      mass: 0.6,
    },
  });

  const opacity = Math.min(1, frame / 30);

  return (
    <AbsoluteFill className="hello-world">
      <div
        className="hello-world__title"
        style={{
          transform: `scale(${scale})`,
          opacity,
        }}
      >
        blxcode + remotion
      </div>
      <div className="hello-world__subtitle">frame {frame}</div>
    </AbsoluteFill>
  );
};
