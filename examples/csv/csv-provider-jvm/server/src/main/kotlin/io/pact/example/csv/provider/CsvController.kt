package io.pact.example.csv.provider

import com.github.javafaker.Faker
import org.apache.commons.csv.CSVFormat
import org.apache.commons.csv.CSVParser
import org.apache.commons.csv.CSVPrinter
import org.springframework.web.bind.annotation.PathVariable
import org.springframework.web.bind.annotation.RequestBody
import org.springframework.web.bind.annotation.RequestMapping
import org.springframework.web.bind.annotation.RequestMethod
import org.springframework.web.bind.annotation.RestController
import java.io.StringReader
import java.io.StringWriter
import java.util.Random
import javax.servlet.http.HttpServletResponse
import kotlin.math.absoluteValue

@RestController
class CsvController {

  @RequestMapping("/reports/{report}.csv", method = [ RequestMethod.POST ], consumes = [ "text/csv; charset=UTF-8" ])
  fun getReport(@PathVariable report: String, @RequestBody data: String, response: HttpServletResponse) {
    val parser = CSVParser(StringReader(data), CSVFormat.EXCEL)
    parser.records
    response.status = 201
  }

  @RequestMapping("/reports/headers/{report}.csv", produces = [ "text/csv; charset=UTF-8" ])
  fun getReportWithHeaders(@PathVariable report: String): String {
    val writer = StringWriter()
    CSVPrinter(writer, CSVFormat.EXCEL).use { printer ->
      printer.printRecord("Name", "Number", "Date")
      generateCsvData(printer)
    }
    return writer.toString()
  }

  @RequestMapping("/reports/{report}.csv", method = [ RequestMethod.GET ], produces = [ "text/csv; charset=UTF-8" ])
  fun getReport(@PathVariable report: String): String {
    val writer = StringWriter()
    CSVPrinter(writer, CSVFormat.EXCEL).use { printer ->
      generateCsvData(printer)
    }
    return writer.toString()
  }

  private fun generateCsvData(printer: CSVPrinter) {
    val random = Random()
    val name = Faker.instance().name()
    val rows = 1 + random.nextInt(100)
    for (row in 1..rows) {
      val num = random.nextInt().absoluteValue
      val month = 1 + random.nextInt(11)
      val day = 1 + random.nextInt(27)
      val year = 1900 + random.nextInt(150)
      printer.printRecord(name.name(), num.toString(), String.format("%4d-%02d-%02d", year, month, day))
    }
  }
}
