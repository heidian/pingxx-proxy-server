/*
  Warnings:

  - Added the required column `appId` to the `Charge` table without a default value. This is not possible if the table is not empty.
  - Added the required column `appId` to the `Refund` table without a default value. This is not possible if the table is not empty.

*/
-- AlterTable
ALTER TABLE `Charge` ADD COLUMN `appId` VARCHAR(191) NOT NULL;

-- AlterTable
ALTER TABLE `Refund` ADD COLUMN `appId` VARCHAR(191) NOT NULL;

-- AddForeignKey
ALTER TABLE `Charge` ADD CONSTRAINT `Charge_appId_fkey` FOREIGN KEY (`appId`) REFERENCES `App`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `Refund` ADD CONSTRAINT `Refund_appId_fkey` FOREIGN KEY (`appId`) REFERENCES `App`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;
