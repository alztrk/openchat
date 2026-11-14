const maximumPdfImageBytes = 32 * 1024 * 1024;
const maximumPdfImageDimension = 4096;

export function createDesignPdf(jpeg: Uint8Array, imageWidth: number, imageHeight: number): Uint8Array {
  if (
    jpeg.length < 4 || jpeg.length > maximumPdfImageBytes || jpeg[0] !== 0xff || jpeg[1] !== 0xd8 ||
    jpeg[jpeg.length - 2] !== 0xff || jpeg[jpeg.length - 1] !== 0xd9 ||
    !Number.isSafeInteger(imageWidth) || !Number.isSafeInteger(imageHeight) ||
    imageWidth < 1 || imageHeight < 1 || imageWidth > maximumPdfImageDimension || imageHeight > maximumPdfImageDimension
  ) throw new Error('The Design PDF image is invalid.');

  const pageScale = Math.min(1, 595 / imageWidth, 842 / imageHeight);
  const pageWidth = Math.max(1, Math.round(imageWidth * pageScale));
  const pageHeight = Math.max(1, Math.round(imageHeight * pageScale));
  const encoder = new TextEncoder();
  const content = encoder.encode(`q\n${pageWidth} 0 0 ${pageHeight} 0 0 cm\n/Im0 Do\nQ\n`);
  const objects: Uint8Array[] = [
    encoder.encode('<< /Type /Catalog /Pages 2 0 R >>'),
    encoder.encode('<< /Type /Pages /Kids [3 0 R] /Count 1 >>'),
    encoder.encode(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${pageWidth} ${pageHeight}] /Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>`),
    concatBytes([
      encoder.encode(`<< /Type /XObject /Subtype /Image /Width ${imageWidth} /Height ${imageHeight} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length ${jpeg.length} >>\nstream\n`),
      jpeg,
      encoder.encode('\nendstream'),
    ]),
    concatBytes([
      encoder.encode(`<< /Length ${content.length} >>\nstream\n`),
      content,
      encoder.encode('endstream'),
    ]),
  ];

  const chunks: Uint8Array[] = [encoder.encode('%PDF-1.4\n% OpenChat Design\n')];
  const offsets = [0];
  let byteLength = chunks[0]?.length ?? 0;
  for (let index = 0; index < objects.length; index += 1) {
    offsets.push(byteLength);
    const objectBytes = concatBytes([encoder.encode(`${index + 1} 0 obj\n`), objects[index] ?? new Uint8Array(), encoder.encode('\nendobj\n')]);
    chunks.push(objectBytes);
    byteLength += objectBytes.length;
  }
  const xrefOffset = byteLength;
  const xref = [`xref\n0 ${objects.length + 1}\n`, '0000000000 65535 f \n'];
  for (const offset of offsets.slice(1)) xref.push(`${String(offset).padStart(10, '0')} 00000 n \n`);
  chunks.push(encoder.encode(`${xref.join('')}trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xrefOffset}\n%%EOF`));
  return concatBytes(chunks);
}

function concatBytes(chunks: readonly Uint8Array[]): Uint8Array {
  const totalLength = chunks.reduce((total, chunk) => total + chunk.length, 0);
  const output = new Uint8Array(totalLength);
  let offset = 0;
  for (const chunk of chunks) {
    output.set(chunk, offset);
    offset += chunk.length;
  }
  return output;
}
