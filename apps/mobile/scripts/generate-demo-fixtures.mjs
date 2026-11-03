import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const fixtures = [
  { source: "3DBenchy.stl", output: "3DBenchy.vlm5", name: "3DBenchy", color: [59, 130, 246] },
  { source: "Flexi-Rex-improved.stl", output: "Flexi-Rex-improved.vlm5", name: "Flexi Rex", color: [255, 107, 69] },
  { source: "NASA-Wrench.stl", output: "NASA-Wrench.vlm5", name: "NASA ISS Wrench", color: [33, 161, 121] },
  { source: "Voron-Stealthburner-main-body.stl", output: "Voron-Stealthburner-main-body.vlm5", name: "Stealthburner", color: [244, 184, 68] },
];

for (const fixture of fixtures) generateFixture(fixture);

function generateFixture(fixture) {
  const sourcePath = resolve(appRoot, `public/demo/${fixture.source}`);
  const outputPath = resolve(appRoot, `public/demo/${fixture.output}`);
  const source = readFileSync(sourcePath);

  if (source.byteLength < 84) throw new Error(`${fixture.source} is incomplete.`);
  const triangleCount = source.readUInt32LE(80);
  if (84 + triangleCount * 50 !== source.byteLength) {
    throw new Error(`${fixture.source} is not a binary STL file.`);
  }

  const vertices = [];
  const indices = new Uint32Array(triangleCount * 3);
  const vertexIndex = new Map();
  const bounds = [Infinity, Infinity, Infinity, -Infinity, -Infinity, -Infinity];

  for (let triangle = 0; triangle < triangleCount; triangle += 1) {
    const triangleOffset = 84 + triangle * 50 + 12;
    for (let corner = 0; corner < 3; corner += 1) {
      const offset = triangleOffset + corner * 12;
      const key = `${source.readUInt32LE(offset)}:${source.readUInt32LE(offset + 4)}:${source.readUInt32LE(offset + 8)}`;
      let index = vertexIndex.get(key);
      if (index === undefined) {
        const x = source.readFloatLE(offset);
        const y = source.readFloatLE(offset + 4);
        const z = source.readFloatLE(offset + 8);
        if (![x, y, z].every(Number.isFinite)) throw new Error(`${fixture.source} contains an invalid vertex.`);
        index = vertices.length / 3;
        vertexIndex.set(key, index);
        vertices.push(x, y, z);
        bounds[0] = Math.min(bounds[0], x);
        bounds[1] = Math.min(bounds[1], y);
        bounds[2] = Math.min(bounds[2], z);
        bounds[3] = Math.max(bounds[3], x);
        bounds[4] = Math.max(bounds[4], y);
        bounds[5] = Math.max(bounds[5], z);
      }
      indices[triangle * 3 + corner] = index;
    }
  }

  const name = Buffer.from(fixture.name, "utf8");
  const headerLength = 72;
  const colorLength = triangleCount * 3;
  const kindLength = triangleCount;
  const objectIdLength = triangleCount * 4;
  const trailerLength = 12 + name.length;
  const output = Buffer.allocUnsafe(
    headerLength
    + vertices.length * 4
    + indices.byteLength
    + colorLength
    + kindLength
    + objectIdLength
    + trailerLength,
  );

  output.write("VLM5", 0, "ascii");
  output.writeUInt32LE(vertices.length / 3, 4);
  output.writeUInt32LE(indices.length, 8);
  bounds.forEach((value, index) => output.writeFloatLE(value, 12 + index * 4));
  output.writeUInt32LE(1, 36);
  output.writeUInt32LE(1, 40);
  output.writeUInt32LE(triangleCount, 44);
  bounds.forEach((value, index) => output.writeFloatLE(value, 48 + index * 4));

  let cursor = headerLength;
  vertices.forEach((value) => { output.writeFloatLE(value, cursor); cursor += 4; });
  indices.forEach((value) => { output.writeUInt32LE(value, cursor); cursor += 4; });
  for (let triangle = 0; triangle < triangleCount; triangle += 1) {
    output[cursor++] = fixture.color[0];
    output[cursor++] = fixture.color[1];
    output[cursor++] = fixture.color[2];
  }
  output.fill(0, cursor, cursor + kindLength);
  cursor += kindLength;
  for (let triangle = 0; triangle < triangleCount; triangle += 1) {
    output.writeUInt32LE(1, cursor);
    cursor += 4;
  }
  output.write("VNM1", cursor, "ascii");
  cursor += 4;
  output.writeUInt32LE(1, cursor);
  cursor += 4;
  output.writeUInt32LE(name.length, cursor);
  cursor += 4;
  name.copy(output, cursor);
  cursor += name.length;

  if (cursor !== output.byteLength) throw new Error(`Generated ${fixture.output} size does not match its payload.`);
  writeFileSync(outputPath, output);

  const dimensions = [bounds[3] - bounds[0], bounds[4] - bounds[1], bounds[5] - bounds[2]];
  console.log(`Generated ${fixture.output}: ${triangleCount.toLocaleString()} triangles · ${dimensions.map((value) => value.toFixed(1)).join(" × ")} mm · ${(output.byteLength / 1_048_576).toFixed(1)} MB`);
}
