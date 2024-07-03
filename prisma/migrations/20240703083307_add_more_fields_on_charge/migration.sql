/*
  Warnings:

  - Added the required column `time_expire` to the `Charge` table without a default value. This is not possible if the table is not empty.

*/
-- AlterTable
ALTER TABLE `Charge` ADD COLUMN `time_expire` INTEGER NOT NULL,
    ADD COLUMN `time_paid` INTEGER NULL;

-- AlterTable
ALTER TABLE `Refund` ADD COLUMN `time_succeed` INTEGER NULL;
