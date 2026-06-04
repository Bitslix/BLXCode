import { AbsoluteFill, Sequence } from "remotion";
import { START_DURATION, StartScene } from "./scenes/Start";
import { MID_DURATION, MidScene } from "./scenes/Mid";
import { END_DURATION, EndScene } from "./scenes/End";
import "../ReleaseTrailer/ReleaseTrailer.css";

const SCENES: { id: string; duration: number; component: React.FC }[] = [
  { id: "start", duration: START_DURATION, component: StartScene },
  { id: "mid", duration: MID_DURATION, component: MidScene },
  { id: "end", duration: END_DURATION, component: EndScene },
];

export const SneakPeekTrailer: React.FC = () => {
  let offset = 0;
  return (
    <AbsoluteFill className="trailer">
      {SCENES.map((scene) => {
        const from = offset;
        offset += scene.duration;
        return (
          <Sequence key={scene.id} from={from} durationInFrames={scene.duration}>
            <scene.component />
          </Sequence>
        );
      })}
    </AbsoluteFill>
  );
};

export const SNEAK_PEEK_TRAILER_DURATION = SCENES.reduce((acc, s) => acc + s.duration, 0);
