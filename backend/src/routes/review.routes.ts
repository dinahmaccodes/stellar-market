import { Router, Response } from "express";
import { PrismaClient } from "@prisma/client";
import { authenticate, AuthRequest } from "../middleware/auth";
import { validate } from "../middleware/validation";
import { asyncHandler } from "../middleware/error";
import {
  createReviewSchema,
  updateReviewSchema,
  getReviewsQuerySchema,
  getReviewByIdParamSchema
} from "../schemas";

const router = Router();
const prisma = new PrismaClient();

// Create a review
router.post("/",
  authenticate,
  validate({ body: createReviewSchema }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { jobId, revieweeId, rating, comment } = req.body;

    // Verify the job exists and is completed
    const job = await prisma.job.findUnique({ where: { id: jobId } });
    if (!job) {
      return res.status(404).json({ error: "Job not found." });
    }
    if (job.status !== "COMPLETED") {
      return res.status(400).json({ error: "Can only review completed jobs." });
    }

    // Verify reviewer is part of the job
    if (job.clientId !== req.userId && job.freelancerId !== req.userId) {
      return res.status(403).json({ error: "Not authorized to review this job." });
    }

    const review = await prisma.review.create({
      data: {
        jobId,
        reviewerId: req.userId!,
        revieweeId,
        rating: typeof rating === 'string' ? parseInt(rating) : rating,
        comment,
      },
      include: {
        reviewer: { select: { id: true, username: true, avatarUrl: true } },
        reviewee: { select: { id: true, username: true, avatarUrl: true } },
      },
    });

    res.status(201).json(review);
  })
);

// Get reviews for a user
router.get("/user/:userId",
  validate({ params: getReviewByIdParamSchema }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { id: userId } = req.params;
    const reviews = await prisma.review.findMany({
      where: { revieweeId: userId },
      include: {
        reviewer: { select: { id: true, username: true, avatarUrl: true } },
        job: { select: { id: true, title: true } },
      },
      orderBy: { createdAt: "desc" },
    });

    const avgRating =
      reviews.length > 0
        ? reviews.reduce((sum, r) => sum + r.rating, 0) / reviews.length
        : 0;

    res.json({ reviews, averageRating: Math.round(avgRating * 100) / 100, totalReviews: reviews.length });
  })
);

// Get all reviews with filtering
router.get("/",
  validate({ query: getReviewsQuerySchema }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { page, limit, jobId, reviewerId, revieweeId, rating } = req.query as any;
    const skip = (page - 1) * limit;

    const where: any = {};
    if (jobId) where.jobId = jobId;
    if (reviewerId) where.reviewerId = reviewerId;
    if (revieweeId) where.revieweeId = revieweeId;
    if (rating) where.rating = rating;

    const [reviews, total] = await Promise.all([
      prisma.review.findMany({
        where,
        include: {
          reviewer: { select: { id: true, username: true, avatarUrl: true } },
          reviewee: { select: { id: true, username: true, avatarUrl: true } },
          job: { select: { id: true, title: true } },
        },
        orderBy: { createdAt: "desc" },
        skip,
        take: limit,
      }),
      prisma.review.count({ where }),
    ]);

    res.json({
      data: reviews,
      total,
      page,
      totalPages: Math.ceil(total / limit),
    });
  })
);

// Get a single review by ID
router.get("/:id",
  authenticate,
  validate({ params: getReviewByIdParamSchema }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { id } = req.params;
    
    const review = await prisma.review.findUnique({
      where: { id },
      include: {
        reviewer: { select: { id: true, username: true, avatarUrl: true } },
        reviewee: { select: { id: true, username: true, avatarUrl: true } },
        job: { select: { id: true, title: true } },
      },
    });

    if (!review) {
      return res.status(404).json({ error: "Review not found." });
    }

    res.json(review);
  })
);

// Update a review
router.put("/:id",
  authenticate,
  validate({
    params: getReviewByIdParamSchema,
    body: updateReviewSchema
  }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { id } = req.params;
    const updateData = req.body;

    const review = await prisma.review.findUnique({
      where: { id },
    });

    if (!review) {
      return res.status(404).json({ error: "Review not found." });
    }
    if (review.reviewerId !== req.userId) {
      return res.status(403).json({ error: "Not authorized to update this review." });
    }

    const updated = await prisma.review.update({
      where: { id },
      data: updateData,
      include: {
        reviewer: { select: { id: true, username: true, avatarUrl: true } },
        reviewee: { select: { id: true, username: true, avatarUrl: true } },
        job: { select: { id: true, title: true } },
      },
    });

    res.json(updated);
  })
);

// Delete a review
router.delete("/:id",
  authenticate,
  validate({ params: getReviewByIdParamSchema }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { id } = req.params;
    
    const review = await prisma.review.findUnique({
      where: { id },
    });

    if (!review) {
      return res.status(404).json({ error: "Review not found." });
    }
    if (review.reviewerId !== req.userId) {
      return res.status(403).json({ error: "Not authorized to delete this review." });
    }

    await prisma.review.delete({ where: { id } });
    res.json({ message: "Review deleted successfully." });
  })
);

export default router;
