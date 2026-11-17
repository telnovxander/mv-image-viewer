// Pack PNG files into one Windows .ico (PNG-compressed entries), without dependencies.
// Usage: node scripts/build-ico.mjs out.ico 16.png 24.png 32.png 48.png 256.png
import { readFileSync, writeFileSync } from "node:fs";

const [output, ...inputs] = process.argv.slice(2);
if (!output || inputs.length === 0) {
  console.error("usage: node scripts/build-ico.mjs out.ico <png>...");
  process.exit(2);
}
const images = inputs.map((path) => {
  const data = readFileSync(path);
  if (data.readUInt32BE(0) !== 0x89504e47) throw new Error(`${path} is not a PNG`);
  return { data, width: data.readUInt32BE(16), height: data.readUInt32BE(20) };
});
const header = Buffer.alloc(6 + 16 * images.length);
header.writeUInt16LE(0, 0); // reserved
header.writeUInt16LE(1, 2); // type: icon
header.writeUInt16LE(images.length, 4);
let offset = header.length;
images.forEach((image, index) => {
  const entry = 6 + 16 * index;
  header.writeUInt8(image.width >= 256 ? 0 : image.width, entry);
  header.writeUInt8(image.height >= 256 ? 0 : image.height, entry + 1);
  header.writeUInt16LE(1, entry + 4); // colour planes
  header.writeUInt16LE(32, entry + 6); // bits per pixel
  header.writeUInt32LE(image.data.length, entry + 8);
  header.writeUInt32LE(offset, entry + 12);
  offset += image.data.length;
});
writeFileSync(output, Buffer.concat([header, ...images.map((image) => image.data)]));
console.log(`Written ${output}: ${images.map((i) => i.width).join(", ")} px`);
