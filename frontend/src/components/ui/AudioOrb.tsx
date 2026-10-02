import React, { useEffect, useRef } from 'react';
import * as THREE from 'three';

interface AudioOrbProps {
  audioLevel?: number; // 0.0 to 1.0
  active?: boolean;
  size?: number; // px
  color?: string;
  glowColor?: string;
}

export const AudioOrb: React.FC<AudioOrbProps> = ({
  audioLevel = 0,
  active = false,
  size = 180,
  color = '#a78bfa',
  glowColor = '#38bdf8',
}) => {
  const mountRef = useRef<HTMLDivElement>(null);
  const audioLevelRef = useRef(audioLevel);
  const activeRef = useRef(active);

  useEffect(() => {
    audioLevelRef.current = audioLevel;
  }, [audioLevel]);

  useEffect(() => {
    activeRef.current = active;
  }, [active]);

  useEffect(() => {
    const container = mountRef.current;
    if (!container) return;

    // 1. Scene, Camera, Renderer
    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
    camera.position.z = 4.2;

    const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true, powerPreference: 'low-power' });
    renderer.setSize(size, size);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    container.appendChild(renderer.domElement);

    // 2. Low-Poly Icosahedron Geometry (~642 vertices)
    const originalGeometry = new THREE.IcosahedronGeometry(1.4, 2);
    const geometry = originalGeometry.clone();
    const positionAttr = geometry.attributes.position;
    const basePositions = new Float32Array(positionAttr.array);

    // 3. Materials
    // Wireframe Mesh for sleek cyber-orb aesthetic
    const wireframeMaterial = new THREE.MeshBasicMaterial({
      color: new THREE.Color(color),
      wireframe: true,
      transparent: true,
      opacity: 0.35,
    });
    const orbMesh = new THREE.Mesh(geometry, wireframeMaterial);
    scene.add(orbMesh);

    // Particle nodes on vertices
    const pointsMaterial = new THREE.PointsMaterial({
      color: new THREE.Color(glowColor),
      size: 0.05,
      transparent: true,
      opacity: 0.85,
    });
    const points = new THREE.Points(geometry, pointsMaterial);
    scene.add(points);

    // 4. Animation loop with smart desktop throttling
    let animationFrameId: number;
    let clock = new THREE.Clock();
    let isPaused = false;

    const handleVisibilityChange = () => {
      if (document.hidden) {
        isPaused = true;
        cancelAnimationFrame(animationFrameId);
      } else {
        isPaused = false;
        clock.getDelta(); // reset delta
        animate();
      }
    };
    document.addEventListener('visibilitychange', handleVisibilityChange);

    const animate = () => {
      if (isPaused) return;

      const time = clock.getElapsedTime();
      const currentLevel = audioLevelRef.current;
      const isActive = activeRef.current;

      // Rotate orb gently
      const rotSpeed = isActive ? 0.8 : 0.25;
      orbMesh.rotation.y = time * rotSpeed;
      orbMesh.rotation.x = Math.sin(time * 0.4) * 0.3;
      points.rotation.y = orbMesh.rotation.y;
      points.rotation.x = orbMesh.rotation.x;

      // Pulse/deform vertices according to live audio levels & gentle sine breathing
      const posArray = positionAttr.array as Float32Array;
      const count = positionAttr.count;

      const pulseFactor = isActive ? (0.35 + currentLevel * 0.8) : 0.08;
      const baseFreq = isActive ? 4.0 : 2.0;

      for (let i = 0; i < count; i++) {
        const i3 = i * 3;
        const ox = basePositions[i3];
        const oy = basePositions[i3 + 1];
        const oz = basePositions[i3 + 2];

        // Spherical breathing displacement
        const wave = Math.sin(time * baseFreq + ox * 3.0 + oy * 2.0) * pulseFactor;
        const scale = 1.0 + wave;

        posArray[i3] = ox * scale;
        posArray[i3 + 1] = oy * scale;
        posArray[i3 + 2] = oz * scale;
      }
      positionAttr.needsUpdate = true;

      // Dynamic opacity
      wireframeMaterial.opacity = isActive ? 0.65 : 0.3;
      pointsMaterial.opacity = isActive ? 0.95 : 0.6;

      renderer.render(scene, camera);
      animationFrameId = requestAnimationFrame(animate);
    };

    animate();

    // 5. Cleanup
    return () => {
      cancelAnimationFrame(animationFrameId);
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      if (container && renderer.domElement) {
        container.removeChild(renderer.domElement);
      }
      originalGeometry.dispose();
      geometry.dispose();
      wireframeMaterial.dispose();
      pointsMaterial.dispose();
      renderer.dispose();
    };
  }, [size, color, glowColor]);

  return (
    <div
      ref={mountRef}
      style={{
        width: `${size}px`,
        height: `${size}px`,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        position: 'relative',
        userSelect: 'none',
      }}
    />
  );
};
