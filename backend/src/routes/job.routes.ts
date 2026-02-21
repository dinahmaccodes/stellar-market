import { Router, Response } from "express";
import { PrismaClient } from "@prisma/client";
import { authenticate, AuthRequest } from "../middleware/auth";

const router = Router();
const prisma = new PrismaClient();

// Get all jobs with optional filters and pagination
router.get("/", async (req: AuthRequest, res: Response) => {
  try {
    const { category, status, search } = req.query;
    const page = Math.max(1, parseInt(req.query.page as string) || 1);
    const limit = Math.min(100, Math.max(1, parseInt(req.query.limit as string) || 10));
    const skip = (page - 1) * limit;

    const where: any = {};
    if (category) where.category = category;
    if (status) where.status = status;
    if (search) {
      where.OR = [
        { title: { contains: search as string, mode: "insensitive" } },
        { description: { contains: search as string, mode: "insensitive" } },
      ];
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
  } catch (error) {
    console.error("Get jobs error:", error);
    res.status(500).json({ error: "Internal server error." });
  }
});

// Get a single job by ID
router.get("/:id", async (req: AuthRequest, res: Response) => {
  try {
    const jobId = req.params.id as string;
    const job = await prisma.job.findUnique({
      where: { id: jobId },
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
      res.status(404).json({ error: "Job not found." });
      return;
    }

    res.json(job);
  } catch (error) {
    console.error("Get job error:", error);
    res.status(500).json({ error: "Internal server error." });
  }
});

// Create a new job
router.post("/", authenticate, async (req: AuthRequest, res: Response) => {
  try {
    const { title, description, budget, category, milestones } = req.body;

    if (!title || !description || !budget || !category) {
      res.status(400).json({ error: "Title, description, budget, and category are required." });
      return;
    }

    const job = await prisma.job.create({
      data: {
        title,
        description,
        budget: parseFloat(budget),
        category,
        clientId: req.userId!,
        milestones: milestones
          ? {
              create: milestones.map((m: any, index: number) => ({
                title: m.title,
                description: m.description,
                amount: parseFloat(m.amount),
                order: index,
              })),
            }
          : undefined,
      },
      include: { milestones: true },
    });

    res.status(201).json(job);
  } catch (error) {
    console.error("Create job error:", error);
    res.status(500).json({ error: "Internal server error." });
  }
});

// Update a job
router.put("/:id", authenticate, async (req: AuthRequest, res: Response) => {
  try {
    const jobId = req.params.id as string;
    const job = await prisma.job.findUnique({ where: { id: jobId } });

    if (!job) {
      res.status(404).json({ error: "Job not found." });
      return;
    }
    if (job.clientId !== req.userId) {
      res.status(403).json({ error: "Not authorized to update this job." });
      return;
    }

    const { title, description, budget, category, status } = req.body;

    const updated = await prisma.job.update({
      where: { id: jobId },
      data: {
        ...(title && { title }),
        ...(description && { description }),
        ...(budget && { budget: parseFloat(budget) }),
        ...(category && { category }),
        ...(status && { status }),
      },
      include: { milestones: true },
    });

    res.json(updated);
  } catch (error) {
    console.error("Update job error:", error);
    res.status(500).json({ error: "Internal server error." });
  }
});

// Delete a job
router.delete("/:id", authenticate, async (req: AuthRequest, res: Response) => {
  try {
    const jobId = req.params.id as string;
    const job = await prisma.job.findUnique({ where: { id: jobId } });

    if (!job) {
      res.status(404).json({ error: "Job not found." });
      return;
    }
    if (job.clientId !== req.userId) {
      res.status(403).json({ error: "Not authorized to delete this job." });
      return;
    }

    await prisma.job.delete({ where: { id: jobId } });
    res.json({ message: "Job deleted successfully." });
  } catch (error) {
    console.error("Delete job error:", error);
    res.status(500).json({ error: "Internal server error." });
  }
});

export default router;
