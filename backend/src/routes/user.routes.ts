import { Router, Response } from "express";
import { PrismaClient } from "@prisma/client";
import { z } from "zod";
import { AuthRequest, authenticate } from "../middleware/auth";
import { validate } from "../middleware/validation";
import { asyncHandler } from "../middleware/error";
import {
  getUserByIdParamSchema,
  updateUserProfileSchema,
  getUsersQuerySchema,
} from "../schemas";

const router = Router();
const prisma = new PrismaClient();

// Zod schema for profile update payload
const updateProfileSchema = z.object({
  username: z
    .string()
    .min(3, "Username must be at least 3 characters")
    .max(30, "Username must be at most 30 characters")
    .regex(/^[a-zA-Z0-9_-]+$/, "Username can only contain letters, numbers, hyphens, and underscores")
    .optional(),
  email: z
    .string()
    .email("Invalid email address")
    .optional()
    .nullable(),
  bio: z
    .string()
    .max(500, "Bio must be at most 500 characters")
    .optional()
    .nullable(),
  avatarUrl: z
    .string()
    .url("Invalid URL for avatar")
    .optional()
    .nullable(),
  role: z
    .enum(["CLIENT", "FREELANCER"], {
      errorMap: () => ({ message: "Role must be either CLIENT or FREELANCER" }),
    })
    .optional(),
});

// GET /api/users/me — return current authenticated user's full profile
router.get("/me", authenticate, async (req: AuthRequest, res: Response) => {
  try {
    const user = await prisma.user.findUnique({
      where: { id: req.userId },
      select: {
        id: true,
        username: true,
        walletAddress: true,
        email: true,
        bio: true,
        avatarUrl: true,
        role: true,
        createdAt: true,
      },
    });

    if (!user) {
      res.status(404).json({ error: "User not found." });
      return;
    }

    res.json(user);
  } catch (error) {
    console.error("Get current user error:", error);
    res.status(500).json({ error: "Internal server error." });
  }
});

// PUT /api/users/me — update current authenticated user's profile
router.put("/me", authenticate, async (req: AuthRequest, res: Response) => {
  try {
    const parsed = updateProfileSchema.safeParse(req.body);

    if (!parsed.success) {
      const errors = parsed.error.errors.map((e) => ({
        field: e.path.join("."),
        message: e.message,
      }));
      res.status(400).json({ error: "Validation failed.", details: errors });
      return;
    }

    const data = parsed.data;

    // Check username uniqueness if being updated
    if (data.username) {
      const existingUser = await prisma.user.findFirst({
        where: {
          username: data.username,
          NOT: { id: req.userId },
        },
      });
      if (existingUser) {
        res.status(409).json({ error: "Username is already taken." });
        return;
      }
    }

    // Check email uniqueness if being updated
    if (data.email) {
      const existingUser = await prisma.user.findFirst({
        where: {
          email: data.email,
          NOT: { id: req.userId },
        },
      });
      if (existingUser) {
        res.status(409).json({ error: "Email is already taken." });
        return;
      }
    }

    const updatedUser = await prisma.user.update({
      where: { id: req.userId },
      data,
      select: {
        id: true,
        username: true,
        walletAddress: true,
        email: true,
        bio: true,
        avatarUrl: true,
        role: true,
        createdAt: true,
      },
    });

    res.json(updatedUser);
  } catch (error) {
    console.error("Update profile error:", error);
    res.status(500).json({ error: "Internal server error." });
  }
});

// Get user profile by ID
router.get(
  "/:id",
  validate({ params: getUserByIdParamSchema }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { id } = req.params;

    const user = await prisma.user.findUnique({
      where: { id },
      select: {
        id: true,
        username: true,
        walletAddress: true,
        bio: true,
        avatarUrl: true,
        role: true,
        createdAt: true,
        reviewsReceived: {
          include: {
            reviewer: {
              select: {
                id: true,
                username: true,
                avatarUrl: true,
              },
            },
          },
          orderBy: { createdAt: "desc" },
        },
        clientJobs: {
          where: { status: "COMPLETED" },
          orderBy: { updatedAt: "desc" },
        },
        freelancerJobs: {
          where: { status: "COMPLETED" },
          orderBy: { updatedAt: "desc" },
        },
      },
    });

    if (!user) {
      return res.status(404).json({ error: "User not found." });
    }

    // Calculate aggregate rating
    const ratings: number[] = user.reviewsReceived.map((r: any) => r.rating);
    const averageRating =
      ratings.length > 0
        ? ratings.reduce((a: number, b: number) => a + b, 0) / ratings.length
        : 0;

    res.json({
      ...user,
      averageRating: parseFloat(averageRating.toFixed(1)),
      reviewCount: ratings.length,
    });
  }),
);

// Get all users with pagination and filtering
router.get(
  "/",
  validate({ query: getUsersQuerySchema }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { page, limit, search, skill } = req.query as any;

    const skip = (page - 1) * limit;

    const where: any = {};

    if (search) {
      where.OR = [
        { username: { contains: search, mode: "insensitive" } },
        { bio: { contains: search, mode: "insensitive" } },
      ];
    }

    if (skill) {
      where.skills = {
        has: skill,
      };
    }

    const [users, total] = await Promise.all([
      prisma.user.findMany({
        where,
        skip,
        take: limit,
        select: {
          id: true,
          username: true,
          walletAddress: true,
          bio: true,
          avatarUrl: true,
          role: true,
          createdAt: true,
        },
        orderBy: { createdAt: "desc" },
      }),
      prisma.user.count({ where }),
    ]);

    res.json({
      users,
      pagination: {
        page,
        limit,
        total,
        pages: Math.ceil(total / limit),
      },
    });
  }),
);

// Update user profile
router.put(
  "/:id",
  validate({
    params: getUserByIdParamSchema,
    body: updateUserProfileSchema,
  }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { id } = req.params;
    const updateData = req.body;

    // Check if user is updating their own profile
    if (req.userId !== id) {
      return res
        .status(403)
        .json({ error: "Not authorized to update this profile." });
    }

    const user = await prisma.user.update({
      where: { id },
      data: updateData,
      select: {
        id: true,
        username: true,
        walletAddress: true,
        email: true,
        bio: true,
        avatarUrl: true,
        role: true,
        skills: true,
        createdAt: true,
      },
    });

    res.json(user);
  }),
);

export default router;
