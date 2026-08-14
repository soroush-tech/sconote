import { jsPDF } from "jspdf";

// Verovio renders A4-proportioned pages (2100×2970 units), so each SVG maps
// straight onto an A4 sheet.
const PAGE_WIDTH_MM = 210;
const PAGE_HEIGHT_MM = 297;
// ~240 DPI: Verovio's glyph <use> structure defeats SVG→vector PDF
// converters, so pages are rasterized instead — crisp at print size.
const RASTER_WIDTH_PX = 2000;

/// Turn Verovio's page SVG strings into a paginated A4 PDF.
export async function scoreToPdf(svgPages) {
  const pdf = new jsPDF({
    orientation: "portrait",
    unit: "mm",
    format: [PAGE_WIDTH_MM, PAGE_HEIGHT_MM],
  });
  // One canvas reused across pages keeps peak memory flat.
  const canvas = document.createElement("canvas");
  canvas.width = RASTER_WIDTH_PX;
  canvas.height = Math.round((RASTER_WIDTH_PX * PAGE_HEIGHT_MM) / PAGE_WIDTH_MM);
  const context = canvas.getContext("2d");
  for (const [page, svg] of svgPages.entries()) {
    if (page > 0) {
      pdf.addPage();
    }
    context.fillStyle = "#ffffff";
    context.fillRect(0, 0, canvas.width, canvas.height);
    context.drawImage(await svgToImage(svg), 0, 0, canvas.width, canvas.height);
    // The explicit alias is load-bearing: jsPDF otherwise de-duplicates
    // images by a sampled content hash, which collides across same-size
    // score pages and silently replaces one page with another.
    pdf.addImage(
      canvas.toDataURL("image/jpeg", 0.92),
      "JPEG",
      0,
      0,
      PAGE_WIDTH_MM,
      PAGE_HEIGHT_MM,
      `score-page-${page}`,
    );
  }
  return pdf.output("blob");
}

async function svgToImage(svg) {
  const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
  try {
    const image = new Image();
    await new Promise((resolve, reject) => {
      image.onload = resolve;
      image.onerror = () => reject(new Error("could not rasterize score page"));
      image.src = url;
    });
    return image;
  } finally {
    URL.revokeObjectURL(url);
  }
}
