import { Composition } from "remotion";
import {
  BLXCode050Announcement,
  BLXCODE_050_ANNOUNCEMENT_DURATION,
} from "./compositions/Announcement050/Announcement050";
import { HelloWorld } from "./compositions/HelloWorld/HelloWorld";
import { ReleaseTrailer, RELEASE_TRAILER_DURATION } from "./compositions/ReleaseTrailer/ReleaseTrailer";
import { SneakPeekTrailer, SNEAK_PEEK_TRAILER_DURATION } from "./compositions/SneakPeekTrailer/SneakPeekTrailer";

const FPS = 30;
const HELLO_DURATION = 150;
const TRAILER_WIDTH = 1920;
const TRAILER_HEIGHT = 1080;
const HELLO_WIDTH = 1280;
const HELLO_HEIGHT = 720;

export const Root: React.FC = () => {
  return (
    <>
      <Composition
        id="HelloWorld"
        component={HelloWorld}
        durationInFrames={HELLO_DURATION}
        fps={FPS}
        width={HELLO_WIDTH}
        height={HELLO_HEIGHT}
      />
      <Composition
        id="BLXCode050Announcement"
        component={BLXCode050Announcement}
        durationInFrames={BLXCODE_050_ANNOUNCEMENT_DURATION}
        fps={FPS}
        width={TRAILER_WIDTH}
        height={TRAILER_HEIGHT}
      />
      <Composition
        id="ReleaseTrailer"
        component={ReleaseTrailer}
        durationInFrames={RELEASE_TRAILER_DURATION}
        fps={FPS}
        width={TRAILER_WIDTH}
        height={TRAILER_HEIGHT}
      />
      <Composition
        id="SneakPeekTrailer"
        component={SneakPeekTrailer}
        durationInFrames={SNEAK_PEEK_TRAILER_DURATION}
        fps={FPS}
        width={TRAILER_WIDTH}
        height={TRAILER_HEIGHT}
      />
    </>
  );
};
