/*
 * DroboOrb3D — loads the bundled public/assets/Drobo.glb as an interactive
 * Three.js scene. Animation is driven by Remotion's deterministic frame
 * counter so renders stay reproducible and do not depend on RAF timing.
 */
import { Canvas, useThree } from "@react-three/fiber";
import { useGLTF } from "@react-three/drei";
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
      const base = new THREE.Color(i % 2 === 0 ? accent : accent2);
      const shade = new THREE.Color(i % 2 === 0 ? "#263243" : "#32283f");
      mat.map = null;
      mat.normalMap = null;
      mat.roughnessMap = null;
      mat.metalnessMap = null;
      mat.metalness = 0.22;
      mat.roughness = 0.64;
      mat.color.copy(base.clone().lerp(shade, i % 2 === 0 ? 0.24 : 0.32));
      mat.emissive.copy(base);
      mat.emissiveIntensity = recording ? 0.12 : 0.055;
      mat.needsUpdate = true;
    });
    invalidate();
  }, [materials, accent, accent2, recording, invalidate]);

  useEffect(() => {
    if (!rootRef.current) return;
    const t = frame / fps;
    rootRef.current.rotation.y = Math.PI * 1.5 + Math.sin(t * 0.6) * 0.4;
    rootRef.current.rotation.x = Math.sin(t * 0.45) * 0.16;
    rootRef.current.rotation.z = Math.sin(t * 0.28) * 0.05;
    rootRef.current.position.y = Math.sin(t * 0.7) * 0.1 - 0.05;
    rootRef.current.updateMatrixWorld(true);
    if (lightRef.current) {
      const breath = Math.sin(t * (recording ? 1.6 : 0.9)) * 0.5 + 0.5;
      lightRef.current.intensity = (recording ? 1.25 : 0.88) * (0.75 + 0.25 * breath);
    }
    if (rimRef.current) {
      const breath = Math.sin(t * (recording ? 1.2 : 0.7) + 1.1) * 0.5 + 0.5;
      rimRef.current.intensity = (recording ? 0.82 : 0.48) * (0.6 + 0.4 * breath);
    }
    materials.forEach((mat, i) => {
      const breath = Math.sin(t * (recording ? 1.6 : 0.9) + i * 0.4) * 0.5 + 0.5;
      mat.emissiveIntensity = (recording ? 0.12 : 0.055) * (0.7 + 0.3 * breath);
    });
    invalidate();
  }, [frame, fps, recording, materials, invalidate]);

  return (
    <group ref={rootRef} scale={2.0} position={[0, -0.15, 0]}>
      <primitive object={gltf.scene} />
      <pointLight ref={lightRef} position={[0, 0.6, 1.4]} intensity={0.88} color={accent} distance={5} />
      <pointLight ref={rimRef} position={[-1.2, -0.4, -1.8]} intensity={0.48} color={accent2} distance={5} />
    </group>
  );
};

type SceneProps = DroboModelProps;

const Scene: React.FC<SceneProps> = (props) => {
  const { accent, accent2 } = props;
  return (
    <>
      <ambientLight intensity={0.56} />
      <directionalLight position={[4, 6, 6]} intensity={1.25} color="#ffffff" />
      <directionalLight position={[-5, 2, 3]} intensity={0.42} color={accent} />
      <pointLight position={[5, 5, 5]} intensity={0.75} color="#ffffff" />
      <pointLight position={[-4, 2, -3]} intensity={0.38} color={accent} />
      <pointLight position={[0, -4, 3]} intensity={0.32} color={accent2} />
      <pointLight position={[3, -2, -1]} intensity={0.28} color={accent2} />
      <Suspense fallback={null}>
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
        dpr={1}
        frameloop="demand"
      >
        <Scene accent={accent} accent2={accent2} recording={recording} />
      </Canvas>
    </div>
  );
};
