import { Router, Response } from "express";
import { PrismaClient } from "@prisma/client";
import { AuthRequest } from "../middleware/auth";
import { validate } from "../middleware/validation";
import { asyncHandler } from "../middleware/error";
import {
  getUserByIdParamSchema,
  updateUserProfileSchema,
  getUsersQuerySchema,
} from "../schemas";

const router = Router();
const prisma = new PrismaClient();

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
