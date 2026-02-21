import { Router, Request, Response } from "express";
import { PrismaClient } from "@prisma/client";
import bcrypt from "bcryptjs";
import jwt from "jsonwebtoken";
import { config } from "../config";
import { validate } from "../middleware/validation";
import { asyncHandler } from "../middleware/error";
import { registerSchema, loginSchema } from "../schemas";

const router = Router();
const prisma = new PrismaClient();

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

export default router;
