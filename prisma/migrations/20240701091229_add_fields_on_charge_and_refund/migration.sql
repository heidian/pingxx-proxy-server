/*
  Warnings:

  - Added the required column `body` to the `Charge` table without a default value. This is not possible if the table is not empty.
  - Added the required column `client_ip` to the `Charge` table without a default value. This is not possible if the table is not empty.
  - Added the required column `currency` to the `Charge` table without a default value. This is not possible if the table is not empty.
  - Added the required column `merchant_order_no` to the `Charge` table without a default value. This is not possible if the table is not empty.
  - Added the required column `paid` to the `Charge` table without a default value. This is not possible if the table is not empty.
  - Added the required column `subject` to the `Charge` table without a default value. This is not possible if the table is not empty.
  - Added the required column `merchant_order_no` to the `Refund` table without a default value. This is not possible if the table is not empty.

*/
-- AlterTable
ALTER TABLE `Charge` ADD COLUMN `body` VARCHAR(191) NOT NULL,
    ADD COLUMN `client_ip` VARCHAR(191) NOT NULL,
    ADD COLUMN `currency` VARCHAR(191) NOT NULL,
    ADD COLUMN `merchant_order_no` VARCHAR(191) NOT NULL,
    ADD COLUMN `paid` BOOLEAN NOT NULL,
    ADD COLUMN `subject` VARCHAR(191) NOT NULL;

-- AlterTable
ALTER TABLE `Refund` ADD COLUMN `merchant_order_no` VARCHAR(191) NOT NULL;
