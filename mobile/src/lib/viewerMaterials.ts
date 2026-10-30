import * as THREE from "three";

export type ViewerRgb = readonly [number, number, number];

export const BAMBU_LIGHTING = Object.freeze({
  ambient: 0.3,
  topDiffuse: 0.48,
  frontDiffuse: 0.18,
  backDiffuse: 0.18,
  topSpecular: 0.075,
  shininess: 20,
  topDirection: [-0.4574957, 0.4574957, 0.7624929] as const,
  frontDirection: [0.6985074, 0.1397015, 0.6985074] as const,
  backDirection: [0.1397015, 0.6985074, 0.6985074] as const,
});

export const BAMBU_PLATE_COLORS = Object.freeze({
  surface: [48, 49, 57] as const,
  grid: [63, 64, 73] as const,
  majorGrid: [73, 75, 85] as const,
  border: [43, 44, 51] as const,
});

const bambuBlackThreshold = 51;

const vertexShader = /* glsl */ `
attribute float volum_object_id;
varying vec3 eye_position;
varying float object_id;

void main() {
  vec4 view_position = modelViewMatrix * vec4(position, 1.0);
  eye_position = view_position.xyz;
  object_id = volum_object_id;
  gl_Position = projectionMatrix * view_position;
}
`;

const bambuFragmentShader = /* glsl */ `
uniform vec3 uniform_color;
uniform vec3 light_top_dir;
uniform vec3 light_front_dir;
uniform vec3 light_back_dir;
uniform float intensity_ambient;
uniform float light_top_diffuse;
uniform float light_front_diffuse;
uniform float light_back_diffuse;
uniform float light_top_specular;
uniform float light_top_shininess;
uniform float selected_object_id;
uniform float isolated_object_id;
uniform vec3 selection_color;

varying vec3 eye_position;
varying float object_id;

void main() {
  if (isolated_object_id > 0.5 && abs(object_id - isolated_object_id) > 0.25) discard;
  vec3 face_normal = cross(dFdx(eye_position), dFdy(eye_position));
  float face_normal_length = length(face_normal);
  vec3 eye_normal = face_normal_length > 0.0
    ? face_normal / face_normal_length
    : vec3(0.0, 0.0, 1.0);
  if (!gl_FrontFacing) eye_normal = -eye_normal;

  float diffuse = intensity_ambient;
  diffuse += max(dot(eye_normal, light_top_dir), 0.0) * light_top_diffuse;
  diffuse += max(dot(eye_normal, light_front_dir), 0.0) * light_front_diffuse;
  diffuse += max(dot(eye_normal, light_back_dir), 0.0) * light_back_diffuse;

  float specular = light_top_specular * pow(
    max(dot(-normalize(eye_position), reflect(-light_top_dir, eye_normal)), 0.0),
    light_top_shininess
  );
  vec3 display_color = uniform_color;
  if (selected_object_id > 0.5 && object_id > 0.5) {
    if (abs(object_id - selected_object_id) < 0.25) {
      display_color = mix(display_color, selection_color, 0.62);
      diffuse += 0.12;
    } else {
      display_color *= 0.42;
      diffuse *= 0.82;
    }
  }
  gl_FragColor = vec4(vec3(specular) + display_color * diffuse, 1.0);
}
`;

const rawVertexShader = /* glsl */ `
void main() {
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

const rawFragmentShader = /* glsl */ `
uniform vec3 uniform_color;

void main() {
  gl_FragColor = vec4(uniform_color, 1.0);
}
`;

function displayRgb(color: ViewerRgb): ViewerRgb {
  return color.every((channel) => channel < bambuBlackThreshold)
    ? [bambuBlackThreshold, bambuBlackThreshold, bambuBlackThreshold]
    : color;
}

function rawRgb(color: ViewerRgb) {
  return new Float32Array(color.map((channel) => channel / 255));
}

function shaderMaterial(parameters: THREE.ShaderMaterialParameters) {
  const material = new THREE.ShaderMaterial({
    depthTest: true,
    depthWrite: true,
    transparent: false,
    ...parameters,
  });
  material.toneMapped = false;
  return material;
}

export function createBambuMaterial(color: ViewerRgb, side: THREE.Side = THREE.DoubleSide) {
  const material = shaderMaterial({
    vertexShader,
    fragmentShader: bambuFragmentShader,
    side,
    uniforms: {
      uniform_color: { value: rawRgb(displayRgb(color)) },
      light_top_dir: { value: new Float32Array(BAMBU_LIGHTING.topDirection) },
      light_front_dir: { value: new Float32Array(BAMBU_LIGHTING.frontDirection) },
      light_back_dir: { value: new Float32Array(BAMBU_LIGHTING.backDirection) },
      intensity_ambient: { value: BAMBU_LIGHTING.ambient },
      light_top_diffuse: { value: BAMBU_LIGHTING.topDiffuse },
      light_front_diffuse: { value: BAMBU_LIGHTING.frontDiffuse },
      light_back_diffuse: { value: BAMBU_LIGHTING.backDiffuse },
      light_top_specular: { value: BAMBU_LIGHTING.topSpecular },
      light_top_shininess: { value: BAMBU_LIGHTING.shininess },
      selected_object_id: { value: 0 },
      isolated_object_id: { value: 0 },
      selection_color: { value: rawRgb([54, 172, 255]) },
    },
  });
  material.name = "Bambu filament";
  material.userData.volumMaterialKind = "filament";
  return material;
}

export function createRawPlateMaterial(color: ViewerRgb, side: THREE.Side = THREE.DoubleSide) {
  const material = shaderMaterial({
    vertexShader: rawVertexShader,
    fragmentShader: rawFragmentShader,
    side,
    uniforms: { uniform_color: { value: rawRgb(color) } },
  });
  material.name = "Bambu plate detail";
  material.userData.volumMaterialKind = "plate-detail";
  return material;
}

export function createPlateSurfaceMaterial() {
  const material = shaderMaterial({
    vertexShader: rawVertexShader,
    fragmentShader: rawFragmentShader,
    side: THREE.FrontSide,
    uniforms: { uniform_color: { value: rawRgb(BAMBU_PLATE_COLORS.surface) } },
  });
  material.name = "Bambu plate surface";
  material.userData.volumMaterialKind = "plate-surface";
  return material;
}
