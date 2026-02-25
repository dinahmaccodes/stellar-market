import { Router, Response } from "express";
import { PrismaClient } from "@prisma/client";
import { authenticate, AuthRequest } from "../middleware/auth";
import { validate } from "../middleware/validation";
import { asyncHandler } from "../middleware/error";
import {
  createApplicationSchema,
  updateApplicationSchema,
  updateApplicationStatusSchema,
  getApplicationsQuerySchema,
  getApplicationByIdParamSchema,
  getJobByIdParamSchema,
} from "../schemas";

const router = Router();
/**
 * @swagger
 * tags:
 *   name: Applications
 *   description: Job application endpoints
 */
const prisma = new PrismaClient();

// Apply for a job
router.post(
  /**
   * @swagger
   * /jobs/{jobId}/apply:
   *   post:
   *     summary: Apply for a job
   *     tags: [Applications]
   *     security:
   *       - bearerAuth: []
   *     parameters:
   *       - in: path
   *         name: jobId
   *         required: true
   *         schema:
   *           type: string
   *         description: Job ID
   *     requestBody:
   *       required: true
   *       content:
   *         application/json:
   *           schema:
   *             $ref: '#/components/schemas/CreateApplicationRequest'
   *           examples:
   *             example:
   *               value:
   *                 proposal: "I am a great fit..."
   *                 estimatedDuration: 14
   *                 bidAmount: 500
   *     responses:
   *       201:
   *         description: Application created
   *         content:
   *           application/json:
   *             schema:
   *               $ref: '#/components/schemas/ApplicationResponse'
   *       400:
   *         description: Job not accepting applications
   *         content:
   *           application/json:
   *             schema:
   *               $ref: '#/components/schemas/ErrorResponse'
   */
  /**
   * @swagger
   * /jobs/{jobId}/applications:
   *   get:
   *     summary: Get applications for a job
   *     tags: [Applications]
   *     security:
   *       - bearerAuth: []
   *     parameters:
   *       - in: path
   *         name: jobId
   *         required: true
   *         schema:
   *           type: string
   *         description: Job ID
   *     responses:
   *       200:
   *         description: List of applications
   *         content:
   *           application/json:
   *             schema:
   *               $ref: '#/components/schemas/ApplicationsResponse'
   */
  "/jobs/:jobId/apply",
  authenticate,
  validate({
    params: getJobByIdParamSchema,
    body: createApplicationSchema,
  }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const jobId = req.params.jobId as string;
    const { proposal, estimatedDuration, bidAmount } = req.body;

    const job = await prisma.job.findUnique({ where: { id: jobId } });
    if (!job) {
      return res.status(404).json({ error: "Job not found." });
    }
    if (job.status !== "OPEN") {
      return res
        .status(400)
        .json({ error: "Job is not accepting applications." });
    }
    if (job.clientId === req.userId) {
      return res.status(400).json({ error: "Cannot apply to your own job." });
    }

    const existing = await prisma.application.findUnique({
      where: { jobId_freelancerId: { jobId, freelancerId: req.userId! } },
    });
    if (existing) {
      return res
        .status(409)
        .json({ error: "You have already applied to this job." });
    }

    const application = await prisma.application.create({
      data: {
        jobId: jobId as string,
        freelancerId: req.userId!,
        proposal,
        estimatedDuration,
        bidAmount,
      },
      include: {
        freelancer: { select: { id: true, username: true, avatarUrl: true } },
      },
    });

    res.status(201).json(application);
  }),
);

// Get applications for a job (paginated)
router.get(
  "/jobs/:jobId/applications",
  authenticate,
  validate({
    params: getJobByIdParamSchema,
    query: getApplicationsQuerySchema,
  }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const jobId = req.params.jobId as string;
    const { page, limit, status } = req.query as any;
    const skip = (page - 1) * limit;

    const where: any = { jobId };
    if (status) {
      where.status = status;
    }

    const [applications, total] = await Promise.all([
      prisma.application.findMany({
        where,
        include: {
          freelancer: {
            select: { id: true, username: true, avatarUrl: true, bio: true },
          },
        },
        orderBy: { createdAt: "desc" },
        skip,
        take: limit,
      }),
      prisma.application.count({ where }),
    ]);

    res.json({
      data: applications,
      total,
      page,
      totalPages: Math.ceil(total / limit),
    });
  }),
);

// Get all applications with filtering
router.get(
  "/",
  authenticate,
  validate({ query: getApplicationsQuerySchema }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const { page, limit, jobId, freelancerId, status } = req.query as any;
    const skip = (page - 1) * limit;

    const where: any = {};
    if (jobId) where.jobId = jobId;
    if (freelancerId) where.freelancerId = freelancerId;
    if (status) where.status = status;

    const [applications, total] = await Promise.all([
      prisma.application.findMany({
        where,
        include: {
          freelancer: { select: { id: true, username: true, avatarUrl: true } },
          job: { select: { id: true, title: true } },
        },
        orderBy: { createdAt: "desc" },
        skip,
        take: limit,
      }),
      prisma.application.count({ where }),
    ]);

    res.json({
      data: applications,
      total,
      page,
      totalPages: Math.ceil(total / limit),
    });
  }),
);

// Update application status (accept/reject)
router.put(
  /**
   * @swagger
   * /applications/{id}/status:
   *   put:
   *     summary: Update application status
   *     tags: [Applications]
   *     security:
   *       - bearerAuth: []
   *     parameters:
   *       - in: path
   *         name: id
   *         required: true
   *         schema:
   *           type: string
   *         description: Application ID
   *     requestBody:
   *       required: true
   *       content:
   *         application/json:
   *           schema:
   *             $ref: '#/components/schemas/UpdateApplicationStatusRequest'
   *     responses:
   *       200:
   *         description: Application status updated
   *         content:
   *           application/json:
   *             schema:
   *               $ref: '#/components/schemas/ApplicationResponse'
   *       403:
   *         description: Not authorized
   *         content:
   *           application/json:
   *             schema:
   *               $ref: '#/components/schemas/ErrorResponse'
   *       404:
   *         description: Application not found
   *         content:
   *           application/json:
   *             schema:
   *               $ref: '#/components/schemas/ErrorResponse'
   */
  "/applications/:id/status",
  authenticate,
  validate({
    params: getApplicationByIdParamSchema,
    body: updateApplicationStatusSchema,
  }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const id = req.params.id as string;
    const { status } = req.body;

    const application = await prisma.application.findUnique({
      where: { id },
      include: { job: true },
    });

    if (!application) {
      return res.status(404).json({ error: "Application not found." });
    }
    if (application.job.clientId !== req.userId) {
      return res.status(403).json({ error: "Not authorized." });
    }

    const updated = await prisma.application.update({
      where: { id },
      data: { status },
    });

    // If accepted, assign freelancer to job and update job status
    if (status === "ACCEPTED") {
      await prisma.job.update({
        where: { id: application.jobId },
        data: {
          freelancerId: application.freelancerId,
          status: "IN_PROGRESS",
        },
      });
    }

    res.json(updated);
  }),
);

// Update application
router.put(
  "/applications/:id",
  authenticate,
  validate({
    params: getApplicationByIdParamSchema,
    body: updateApplicationSchema,
  }),
  asyncHandler(async (req: AuthRequest, res: Response) => {
    const id = req.params.id as string;
    const updateData = req.body;

    const application = await prisma.application.findUnique({
      where: { id },
      include: { job: true },
    });

    if (!application) {
      return res.status(404).json({ error: "Application not found." });
    }
    if (application.freelancerId !== req.userId) {
      return res
        .status(403)
        .json({ error: "Not authorized to update this application." });
    }

    const updated = await prisma.application.update({
      where: { id },
      data: updateData,
      include: {
        freelancer: { select: { id: true, username: true, avatarUrl: true } },
      },
    });

    res.json(updated);
  }),
);

export default router;
