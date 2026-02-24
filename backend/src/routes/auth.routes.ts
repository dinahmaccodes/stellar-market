import { Router, Request, Response } from "express";
import { PrismaClient } from "@prisma/client";
import bcrypt from "bcryptjs";
import jwt from "jsonwebtoken";
import { config } from "../config";
import { validate } from "../middleware/validation";
import { authenticate, AuthRequest } from "../middleware/auth";
import { asyncHandler } from "../middleware/error";
import {
  registerSchema,
  loginSchema,
  forgotPasswordSchema,
  resetPasswordSchema,
  verifyEmailParamSchema,
} from "../schemas";
import { generateToken, hashToken } from "../utils/token";
import { sendPasswordResetEmail, sendVerificationEmail } from "../utils/email";

const router = Router();
const prisma = new PrismaClient();

const RESET_TOKEN_EXPIRY_MS = 60 * 60 * 1000; // 1 hour

// Register a new user
router.post(
  "/register",
  validate({ body: registerSchema }),
  asyncHandler(async (req: Request, res: Response) => {
    const { stellarAddress, email, name, password } = req.body;

    const existingUser = await prisma.user.findFirst({
      where: {
        OR: [
          { walletAddress: stellarAddress },
          { username: name },
          ...(email ? [{ email }] : []),
        ],
      },
    });

    if (existingUser) {
      return res.status(409).json({ error: "User already exists." });
    }

    const hashedPassword = password ? await bcrypt.hash(password, 10) : null;

    const user = await prisma.user.create({
      data: {
        walletAddress: stellarAddress,
        email,
        username: name,
        password: hashedPassword,
        role: "FREELANCER",
      },
    });

    const token = jwt.sign({ userId: user.id }, config.jwtSecret, {
      expiresIn: "7d",
    });

    res.status(201).json({
      user: {
        id: user.id,
        walletAddress: user.walletAddress,
        username: user.username,
        email: user.email,
        role: user.role,
      },
      token,
    });
  }),
);

// Login
router.post(
  "/login",
  validate({ body: loginSchema }),
  asyncHandler(async (req: Request, res: Response) => {
    const { email, password } = req.body;

    const user = await prisma.user.findUnique({ where: { email } });

    if (!user || !user.password) {
      return res.status(401).json({ error: "Invalid credentials." });
    }

    const validPassword = await bcrypt.compare(password, user.password);
    if (!validPassword) {
      return res.status(401).json({ error: "Invalid credentials." });
    }

    const token = jwt.sign({ userId: user.id }, config.jwtSecret, {
      expiresIn: "7d",
    });

    res.json({
      user: {
        id: user.id,
        walletAddress: user.walletAddress,
        username: user.username,
        email: user.email,
        role: user.role,
      },
      token,
    });
  }),
);

// Forgot password — generates hashed reset token, sends email
router.post(
  "/forgot-password",
  validate({ body: forgotPasswordSchema }),
  asyncHandler(async (req: Request, res: Response) => {
    const { email } = req.body;

    const user = await prisma.user.findUnique({ where: { email } });

    // Always return success to prevent email enumeration
    if (!user) {
      return res.json({ message: "If the email exists, a reset link has been sent." });
    }

    const rawToken = generateToken();
    const hashed = hashToken(rawToken);

    await prisma.user.update({
      where: { id: user.id },
      data: {
        passwordResetToken: hashed,
        passwordResetExpiry: new Date(Date.now() + RESET_TOKEN_EXPIRY_MS),
      },
    });

    await sendPasswordResetEmail(email, rawToken);

    res.json({ message: "If the email exists, a reset link has been sent." });
  }),
);

// Reset password — validates token + expiry, updates password
router.post(
  "/reset-password",
  validate({ body: resetPasswordSchema }),
  asyncHandler(async (req: Request, res: Response) => {
    const { token, password } = req.body;

    const hashed = hashToken(token);

    const user = await prisma.user.findFirst({
      where: {
        passwordResetToken: hashed,
        passwordResetExpiry: { gt: new Date() },
      },
    });

    if (!user) {
      return res.status(400).json({ error: "Invalid or expired reset token." });
    }

    const hashedPassword = await bcrypt.hash(password, 10);

    await prisma.user.update({
      where: { id: user.id },
      data: {
        password: hashedPassword,
        passwordResetToken: null,
        passwordResetExpiry: null,
      },
    });

    res.json({ message: "Password has been reset successfully." });
  }),
);

// Send verification email — requires authentication
router.post(
  "/send-verification",
  authenticate,
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const user = await prisma.user.findUnique({
      where: { id: req.userId },
    });

    if (!user || !user.email) {
      return res.status(400).json({ error: "No email address on account." });
    }

    if (user.emailVerified) {
      return res.status(400).json({ error: "Email is already verified." });
    }

    const rawToken = generateToken();
    const hashed = hashToken(rawToken);

    await prisma.user.update({
      where: { id: user.id },
      data: { emailVerificationToken: hashed },
    });

    await sendVerificationEmail(user.email, rawToken);

    res.json({ message: "Verification email sent." });
  }),
);

// Verify email — validates token and marks email as verified
router.get(
  "/verify-email/:token",
  validate({ params: verifyEmailParamSchema }),
  asyncHandler(async (req: Request, res: Response) => {
    const { token } = req.params;

    const hashed = hashToken(token);

    const user = await prisma.user.findFirst({
      where: { emailVerificationToken: hashed },
    });

    if (!user) {
      return res.status(400).json({ error: "Invalid verification token." });
    }

    await prisma.user.update({
      where: { id: user.id },
      data: {
        emailVerified: true,
        emailVerificationToken: null,
      },
    });

    res.json({ message: "Email verified successfully." });
  }),
);

export default router;
