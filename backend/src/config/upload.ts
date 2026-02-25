import multer from "multer";
import path from "path";
import fs from "fs";
import crypto from "crypto";

// Allowed MIME types
export const ALLOWED_MIME_TYPES = [
  "application/pdf", // PDF
  "image/jpeg", // JPG
  "image/png", // PNG
  "video/mp4", // MP4
  "application/zip", // ZIP
  "application/x-zip-compressed", // ZIP alternative
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document", // DOCX
];

// Max file size: 10MB
export const MAX_FILE_SIZE = 10 * 1024 * 1024;

// Upload directory
const UPLOAD_DIR =
  process.env.UPLOAD_DIR || path.join(__dirname, "../../uploads");

// Ensure upload directory exists
if (!fs.existsSync(UPLOAD_DIR)) {
  fs.mkdirSync(UPLOAD_DIR, { recursive: true });
}

// Configure storage
const storage = multer.diskStorage({
  destination: (req, file, cb) => {
    cb(null, UPLOAD_DIR);
  },
  filename: (req, file, cb) => {
    // Generate unique filename with timestamp and random hash
    const uniqueSuffix = `${Date.now()}-${crypto.randomBytes(6).toString("hex")}`;
    const ext = path.extname(file.originalname);
    cb(null, `${uniqueSuffix}${ext}`);
  },
});

// File filter for validation
const fileFilter = (
  req: Express.Request,
  file: Express.Multer.File,
  cb: multer.FileFilterCallback,
) => {
  // Check MIME type
  if (!ALLOWED_MIME_TYPES.includes(file.mimetype)) {
    return cb(
      new Error(
        `Invalid file type. Allowed types: PDF, JPG, PNG, MP4, ZIP, DOCX`,
      ),
    );
  }

  cb(null, true);
};

// Configure multer
export const upload = multer({
  storage,
  fileFilter,
  limits: {
    fileSize: MAX_FILE_SIZE,
  },
});

export { UPLOAD_DIR };
