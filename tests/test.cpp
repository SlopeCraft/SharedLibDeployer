#include <iostream>
#include <omp.h>
#include <zip.h>
#ifdef QT_SUPPORT
#include <QCoreApplication>
#include <QMainWindow>
#endif

int main(int argc, char **argv) {

#ifdef QT_SUPPORT
  QCoreApplication qapp{argc, argv};

  QMainWindow wind;
  wind.show();
#endif

  omp_set_num_threads(20);

  zip_close(nullptr);

  std::cout << "DLLDeployer!" << std::endl;

#ifdef QT_SUPPORT
  return qapp.exec();
#else
  return 0;
#endif
}