process.env.PACT_DO_NOT_TRACK = 'true';

module.exports = {
  testEnvironment: 'node',
  testMatch: ['**/src/**/*.test.js'],
  testTimeout: 30000,
};
