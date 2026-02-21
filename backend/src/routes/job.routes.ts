import { Router, Response } from "express";
import { PrismaClient } from "@prisma/client";
import { authenticate, AuthRequest } from "../middleware/auth";
import { validate } from "../middleware/validation";
import { asyncHandler } from "../middleware/error";
import {
  createJobSchema,
  updateJobSchema,
  getJobsQuerySchema,
  getJobByIdParamSchema,
  updateJobStatusSchema
} from "../schemas";

const router = Router();
const prisma = new PrismaClient();

// Get all jobs with optional filters and pagination
router.get("/",
  validate({ query: getJobsQuerySchema }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { page, limit, search, skill, status, minBudget, maxBudget, clientId } = req.query as any;
    const skip = (page - 1) * limit;

    const where: any = {};
    
    if (search) {
      where.OR = [
        { title: { contains: search, mode: "insensitive" } },
        { description: { contains: search, mode: "insensitive" } },
      ];
    }
    
    if (skill) {
      where.skills = {
        has: skill,
      };
    }
    
    if (status) {
      where.status = status;
    }
    
    if (minBudget || maxBudget) {
      where.budget = {};
      if (minBudget) where.budget.gte = minBudget;
      if (maxBudget) where.budget.lte = maxBudget;
    }
    
    if (clientId) {
      where.clientId = clientId;
    }

    const [jobs, total] = await Promise.all([
      prisma.job.findMany({
        where,
        include: {
          client: { select: { id: true, username: true, avatarUrl: true } },
          freelancer: { select: { id: true, username: true, avatarUrl: true } },
          milestones: true,
          _count: { select: { applications: true } },
        },
        orderBy: { createdAt: "desc" },
        skip,
        take: limit,
      }),
      prisma.job.count({ where }),
    ]);

    res.json({
      data: jobs,
      total,
      page,
      totalPages: Math.ceil(total / limit),
    });
  })
);

// Get a single job by ID
router.get("/:id",
  validate({ params: getJobByIdParamSchema }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { id } = req.params;
    const job = await prisma.job.findUnique({
      where: { id },
      include: {
        client: { select: { id: true, username: true, avatarUrl: true, bio: true } },
        freelancer: { select: { id: true, username: true, avatarUrl: true, bio: true } },
        milestones: { orderBy: { order: "asc" } },
        applications: {
          include: {
            freelancer: { select: { id: true, username: true, avatarUrl: true } },
          },
        },
      },
    });

    if (!job) {
      return res.status(404).json({ error: "Job not found." });
    }

    res.json(job);
  })
);

// Create a new job
router.post("/",
  authenticate,
  validate({ body: createJobSchema }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { title, description, budget, skills, deadline } = req.body;

    const job = await prisma.job.create({
      data: {
        title,
        description,
        budget,
        skills,
        deadline: new Date(deadline),
        clientId: req.userId!,
      },
      include: { milestones: true },
    });

    res.status(201).json(job);
  })
);

// Update a job
router.put("/:id",
  authenticate,
  validate({
    params: getJobByIdParamSchema,
    body: updateJobSchema
  }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { id } = req.params;
    const job = await prisma.job.findUnique({ where: { id } });

    if (!job) {
      return res.status(404).json({ error: "Job not found." });
    }
    if (job.clientId !== req.userId) {
      return res.status(403).json({ error: "Not authorized to update this job." });
    }

    const updateData = req.body;
    if (updateData.deadline) {
      updateData.deadline = new Date(updateData.deadline);
    }

    const updated = await prisma.job.update({
      where: { id },
      data: updateData,
      include: { milestones: true },
    });

    res.json(updated);
  })
);

// Delete a job
router.delete("/:id",
  authenticate,
  validate({ params: getJobByIdParamSchema }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { id } = req.params;
    const job = await prisma.job.findUnique({ where: { id } });

    if (!job) {
      return res.status(404).json({ error: "Job not found." });
    }
    if (job.clientId !== req.userId) {
      return res.status(403).json({ error: "Not authorized to delete this job." });
    }

    await prisma.job.delete({ where: { id } });
    res.json({ message: "Job deleted successfully." });
  })
);

// Update job status
router.patch("/:id/status",
  authenticate,
  validate({
    params: getJobByIdParamSchema,
    body: updateJobStatusSchema
  }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { id } = req.params;
    const { status } = req.body;
    
    const job = await prisma.job.findUnique({ where: { id } });

    if (!job) {
      return res.status(404).json({ error: "Job not found." });
    }
    if (job.clientId !== req.userId) {
      return res.status(403).json({ error: "Not authorized to update this job." });
    }

    const updated = await prisma.job.update({
      where: { id },
      data: { status },
      include: { milestones: true },
    });

    res.json(updated);
  })
);

export default router;
