/*
 * DroboOrb3D — loads the bundled public/assets/Drobo.glb as an interactive
 * Three.js scene. Uses @react-three/fiber with `useFrame` for animation
 * (R3F's RAF loop) plus a hook that syncs the canvas transform with
 * Remotion's deterministic frame counter for stills.
 */
import { Canvas, useFrame, useThree } from "@react-three/fiber";
import { Environment, Lightformer, useGLTF } from "@react-three/drei";
import { useCurrentFrame, useVideoConfig, staticFile } from "remotion";
import { Suspense, useEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import "./DroboOrb3D.css";

useGLTF.preload(staticFile("assets/Drobo.glb"));

type DroboModelProps = {
  accent: string;
  accent2: string;
  recording: boolean;
};

const DroboModel: React.FC<DroboModelProps> = ({ accent, accent2, recording }) => {
  const gltf = useGLTF(staticFile("assets/Drobo.glb"));
  const rootRef = useRef<THREE.Group>(null);
  const lightRef = useRef<THREE.PointLight>(null);
  const rimRef = useRef<THREE.PointLight>(null);
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const { invalidate } = useThree();

  const materials = useMemo(() => {
    const list: THREE.MeshStandardMaterial[] = [];
    gltf.scene.traverse((obj) => {
      const mesh = obj as THREE.Mesh;
      if (mesh.isMesh) {
        const mat = mesh.material as THREE.MeshStandardMaterial;
        if (mat && !list.includes(mat)) {
          list.push(mat);
        }
      }
    });
    return list;
  }, [gltf]);

  useEffect(() => {
    materials.forEach((mat, i) => {
      mat.map = null;
      mat.normalMap = null;
      mat.roughnessMap = null;
      mat.metalnessMap = null;
      mat.metalness = 0.45;
      mat.roughness = 0.38;
      mat.color.set(i % 2 === 0 ? accent : accent2);
      mat.emissive.set(i % 2 === 0 ? accent : accent2);
      mat.emissiveIntensity = recording ? 0.55 : 0.2;
      mat.needsUpdate = true;
    });
    invalidate();
  }, [materials, accent, accent2, recording, invalidate]);

  useEffect(() => {
    if (!rootRef.current) return;
    const t = frame / fps;
    rootRef.current.rotation.y = Math.PI + Math.sin(t * 0.6) * 0.4;
    rootRef.current.rotation.x = Math.sin(t * 0.45) * 0.16;
    rootRef.current.rotation.z = Math.sin(t * 0.28) * 0.05;
    rootRef.current.position.y = Math.sin(t * 0.7) * 0.1 - 0.05;
    rootRef.current.updateMatrixWorld(true);
    if (lightRef.current) {
      const breath = Math.sin(t * (recording ? 1.6 : 0.9)) * 0.5 + 0.5;
      lightRef.current.intensity = (recording ? 1.8 : 1.2) * (0.75 + 0.25 * breath);
    }
    if (rimRef.current) {
      const breath = Math.sin(t * (recording ? 1.2 : 0.7) + 1.1) * 0.5 + 0.5;
      rimRef.current.intensity = (recording ? 1.1 : 0.65) * (0.6 + 0.4 * breath);
    }
    materials.forEach((mat, i) => {
      const breath = Math.sin(t * (recording ? 1.6 : 0.9) + i * 0.4) * 0.5 + 0.5;
      mat.emissiveIntensity = (recording ? 0.55 : 0.2) * (0.7 + 0.3 * breath);
    });
    invalidate();
  }, [frame, fps, recording, materials, invalidate]);

  useFrame(({ clock }) => {
    if (!rootRef.current) return;
    const t = clock.getElapsedTime();
    rootRef.current.rotation.y = Math.PI + Math.sin(t * 0.6) * 0.4;
    rootRef.current.rotation.x = Math.sin(t * 0.45) * 0.16;
    rootRef.current.rotation.z = Math.sin(t * 0.28) * 0.05;
    rootRef.current.position.y = Math.sin(t * 0.7) * 0.1 - 0.05;
  });

  return (
    <group ref={rootRef} scale={2.0} position={[0, -0.15, 0]}>
      <primitive object={gltf.scene} />
      <pointLight ref={lightRef} position={[0, 0.6, 1.4]} intensity={1.2} color={accent} distance={5} />
      <pointLight ref={rimRef} position={[-1.2, -0.4, -1.8]} intensity={0.65} color={accent2} distance={5} />
    </group>
  );
};

type SceneProps = DroboModelProps;

const Scene: React.FC<SceneProps> = (props) => {
  return (
    <>
      <ambientLight intensity={0.85} />
      <directionalLight position={[4, 6, 6]} intensity={1.6} color="#ffffff" />
      <directionalLight position={[-5, 2, 3]} intensity={0.7} color="#7dcfff" />
      <pointLight position={[5, 5, 5]} intensity={1.2} color="#ffffff" />
      <pointLight position={[-4, 2, -3]} intensity={0.6} color="#7dcfff" />
      <pointLight position={[0, -4, 3]} intensity={0.5} color="#ff79c6" />
      <pointLight position={[3, -2, -1]} intensity={0.5} color="#bd93f9" />
      <Suspense fallback={null}>
        <Environment background={false} resolution={256} frames={1}>
          <Lightformer
            form="rect"
            intensity={3}
            color="#bd93f9"
            position={[0, 4, 6]}
            scale={[10, 1, 1]}
            target={[0, 0, 0]}
          />
          <Lightformer
            form="rect"
            intensity={2}
            color="#7dcfff"
            position={[-6, 1, 4]}
            scale={[1, 8, 1]}
            target={[0, 0, 0]}
          />
          <Lightformer
            form="ring"
            intensity={1.4}
            color="#ff79c6"
            position={[0, -3, 4]}
            scale={[6, 6, 1]}
            target={[0, 0, 0]}
          />
          <Lightformer
            form="rect"
            intensity={1.1}
            color="#ffffff"
            position={[6, 3, 2]}
            scale={[3, 3, 1]}
            target={[0, 0, 0]}
          />
        </Environment>
        <DroboModel {...props} />
      </Suspense>
    </>
  );
};

export type DroboOrb3DProps = {
  accent?: string;
  accent2?: string;
  recording?: boolean;
  size?: number;
};

export const DroboOrb3D: React.FC<DroboOrb3DProps> = ({
  accent = "#bd93f9",
  accent2 = "#7dcfff",
  recording = false,
  size = 540,
}) => {
  return (
    <div className="drobo-orb" style={{ width: size, height: size }}>
      <Canvas
        camera={{ position: [0, 0.15, 2.6], fov: 28 }}
        gl={{ antialias: true, alpha: true, preserveDrawingBuffer: true }}
        dpr={[1, 2]}
        frameloop="always"
      >
        <Scene accent={accent} accent2={accent2} recording={recording} />
      </Canvas>
    </div>
  );
};
